use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    path::PathBuf,
};

use copypasta::ClipboardContext;
use directories::UserDirs;
use et_wrapper::{ExiftoolEntry, TagEntry};

pub mod et_wrapper;

#[derive(Default)]
pub enum MainInput {
    #[default]
    Main,
    Filter,
    BinarySaveDialog,
}

pub struct BinarySaveDialog {
    pub fname: String,
    pub fext: String,
    pub status: Result<String, String>,
    pub editing_fname: bool,
}

impl Default for BinarySaveDialog {
    fn default() -> Self {
        Self {
            fname: String::new(),
            fext: String::from("jpeg"),
            status: Ok(String::from(
                "File will be saved in Downloads. You probably want a .jpeg.",
            )),
            editing_fname: true,
        }
    }
}

#[derive(Default)]
pub struct CompareData {
    pub mode: Option<bool>,
    pub data: Vec<(TagEntry, Vec<Option<TagEntry>>)>,
}

pub struct MainState {
    pub current_file: PathBuf,
    pub show_details: bool,
    pub binary_save_dialog: Option<BinarySaveDialog>,
    pub filter: String,
    pub num_entries_shown: usize,
    pub et_data: Vec<ExiftoolEntry>,
    pub current_file_index: usize,
    pub data_display_mode: DataDisplayMode,
    pub scroll_offset: (u16, u16),
    pub cursor: usize,
    user_dirs: UserDirs,
    pub log_msg: Option<Result<String, String>>,
    multiple_files_input: Option<Vec<PathBuf>>,
    pub compare_data: CompareData,
}

impl MainState {
    fn new(image_path: PathBuf) -> std::io::Result<Self> {
        let et_data = et_wrapper::run(vec![image_path.clone()], false)?;
        let num_entries_shown = et_data[0].tag_entries.len();

        Ok(Self {
            current_file: image_path,
            show_details: false,
            binary_save_dialog: None,
            filter: String::new(),
            num_entries_shown,
            et_data,
            current_file_index: 0,
            data_display_mode: Default::default(),
            scroll_offset: (0, 0),
            cursor: 0,
            user_dirs: UserDirs::new().expect("Failed to locate user home dir!"),
            log_msg: None,
            multiple_files_input: None,
            compare_data: Default::default(),
        })
    }

    fn new_multiple_files(input: Vec<PathBuf>) -> Self {
        Self {
            current_file: PathBuf::new(),
            show_details: false,
            binary_save_dialog: None,
            filter: String::new(),
            num_entries_shown: 0,
            et_data: Vec::new(),
            current_file_index: 0,
            data_display_mode: Default::default(),
            scroll_offset: (0, 0),
            cursor: 0,
            user_dirs: UserDirs::new().expect("Failed to locate user home dir!"),
            log_msg: None,
            multiple_files_input: Some(input),
            compare_data: Default::default(),
        }
    }

    pub fn read_multiple_files(&mut self, recursive: bool) -> std::io::Result<()> {
        let input_files = self.multiple_files_input.take().unwrap();
        self.et_data = et_wrapper::run(input_files, recursive)?;
        self.num_entries_shown = self.et_data[0].tag_entries.len();
        self.current_file = self.et_data[0].file_name.clone();
        self.calculate_compare_data();
        Ok(())
    }

    pub fn scrollv(&mut self, delta: i8) {
        if delta < 0 {
            self.cursor = self.cursor.saturating_sub(-delta as usize);
        } else {
            self.cursor = self.cursor.saturating_add(delta as usize);
            self.cursor = self.cursor.min(self.num_entries_shown.saturating_sub(1));
        }
    }

    pub fn scrollv_drag_cursor(&mut self, delta: i8) {
        if delta < 0 {
            self.scroll_offset.0 = self.scroll_offset.0.saturating_sub(-delta as u16);
            self.cursor = self.cursor.saturating_sub(-delta as usize);
        } else {
            self.scroll_offset.0 = self.scroll_offset.0.saturating_add(delta as u16);
            self.cursor = self.cursor.saturating_add(delta as usize);
            self.cursor = self.cursor.min(self.num_entries_shown.saturating_sub(1));
        }
    }

    pub fn scrollh(&mut self, delta: i8) {
        if delta < 0 {
            self.scroll_offset.1 = self.scroll_offset.1.saturating_sub(-delta as u16);
        } else {
            self.scroll_offset.1 = self.scroll_offset.1.saturating_add(delta as u16);
        }
    }

    /// Will return a 'key entry' for compare view
    pub fn selected_entry(&self) -> Option<&TagEntry> {
        if let Some(only_diff) = self.compare_data.mode {
            let check_filter = |v: &Vec<Option<TagEntry>>| {
                self.filter.is_empty()
                    || v.iter()
                        .any(|v| v.as_ref().is_some_and(|v| v.check_filter(&self.filter)))
            };

            let check_diff = |v: &Vec<Option<TagEntry>>| {
                if !only_diff {
                    true
                } else {
                    let first = &v[0];
                    !v.iter().all(|entry| {
                        (entry.is_none() && first.is_none())
                            || entry
                                .as_ref()
                                .is_some_and(|e| first.as_ref().is_some_and(|f| e == f))
                    })
                }
            };

            self.compare_data
                .data
                .iter()
                .filter(|ee| check_filter(&ee.1) && check_diff(&ee.1))
                .map(|entry| entry.1[self.current_file_index].as_ref())
                .nth(self.cursor)
                .unwrap_or(None)
        } else {
            self.et_data[self.current_file_index]
                .tag_entries
                .iter()
                .filter(|ee| self.filter.is_empty() || ee.check_filter(&self.filter))
                .nth(self.cursor)
        }
    }

    pub fn try_save_binary(&mut self) -> Result<(), ()> {
        let path = {
            let dialog = self
                .binary_save_dialog
                .as_mut()
                .expect("Something went wrong while trying to save binary data!");
            if dialog.fname.is_empty() {
                dialog.status = Err(String::from("Please enter a name."));
                return Err(());
            }
            if dialog.fext.is_empty() {
                dialog.status = Err(String::from("Please enter an extension."));
                return Err(());
            }
            if dialog.fext.starts_with(".") {
                dialog.fext.remove(0);
            }
            let path = self
                .user_dirs
                .download_dir()
                .expect("Failed to obtain a downloads dir!");
            let fname = dialog.fname.clone() + "." + &dialog.fext;
            let path = path.join(fname);
            if path.exists() {
                dialog.status = Err(String::from("File with this name already exists!"));
                return Err(());
            }
            path
        };
        let entry = self.selected_entry().unwrap();
        let binary = match entry.get_binary(&self.current_file) {
            Ok(binary) => binary,
            Err(_) => {
                return Err(());
            }
        };
        let mut out = File::create_new(&path).expect("Failed to create a new file.");
        out.write_all(&binary)
            .expect("Failed to write binary data.");
        self.log_msg = Some(Ok(format!("Succesfully saved at {}", path.display())));
        Ok(())
    }

    pub fn is_multiple_files(&self) -> bool {
        self.et_data.len() > 1
    }

    fn calculate_compare_data(&mut self) {
        let mut keys = HashSet::new();
        let mut data = Vec::new();
        for file_data in self.et_data.iter() {
            let file_entries = file_data
                .tag_entries
                .iter()
                .map(|e| (e.as_key(), e.clone()))
                .collect::<HashMap<_, _>>();

            for k in file_entries.keys() {
                keys.insert(k.clone());
            }

            data.push(file_entries);
        }

        let mut res: Vec<(TagEntry, Vec<Option<TagEntry>>)> = vec![];

        for key in keys.iter() {
            let mut main_val = None;
            let values: Vec<Option<TagEntry>> = data
                .iter()
                .map(|m| {
                    let val = m.get(key);
                    if val.is_some() && main_val.is_none() {
                        main_val = val.cloned();
                    }
                    val.cloned()
                })
                .collect();
            res.push((main_val.unwrap(), values));
        }

        self.compare_data.data = res;
    }
}

pub enum Screen {
    Main(MainInput),
    Help,
    MiltipleFilesStart,
}

impl Default for Screen {
    fn default() -> Self {
        Self::Main(Default::default())
    }
}

#[derive(Default)]
pub struct DataDisplayMode {
    pub short: bool,
    pub numerical: bool,
}

pub struct App {
    pub screen: Screen,
    pub main_state: MainState,
    pub clipboard: ClipboardContext,
}

impl App {
    pub fn new(image_path: PathBuf) -> std::io::Result<Self> {
        Ok(Self {
            screen: Default::default(),
            main_state: MainState::new(image_path)?,
            clipboard: copypasta::ClipboardContext::new()
                .expect("Failed to obtain a clipboard context"),
        })
    }

    pub fn new_multiple_files(input: Vec<PathBuf>) -> std::io::Result<Self> {
        if input.iter().filter(|p| p.is_dir()).any(|p| {
            std::fs::read_dir(p)
                .unwrap()
                .any(|p| p.unwrap().path().is_dir())
        }) {
            Ok(Self {
                screen: Screen::MiltipleFilesStart,
                main_state: MainState::new_multiple_files(input),
                clipboard: copypasta::ClipboardContext::new()
                    .expect("Failed to obtain a clipboard context"),
            })
        } else {
            let mut main_state = MainState::new_multiple_files(input);
            main_state.read_multiple_files(false)?;
            Ok(Self {
                screen: Default::default(),
                main_state,
                clipboard: copypasta::ClipboardContext::new()
                    .expect("Failed to obtain a clipboard context"),
            })
        }
    }
}
