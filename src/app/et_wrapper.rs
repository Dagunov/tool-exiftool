use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum EtVal {
    String(String),
    Array(Vec<Value>),
}

impl EtVal {
    fn check_filter(&self, filter: &str) -> bool {
        match &self {
            EtVal::String(s) => s.to_lowercase().contains(&filter),
            EtVal::Array(vec) => {
                let mut res = false;
                for v in vec {
                    if v.to_string().contains(&filter) {
                        res = true;
                        break;
                    }
                }
                res
            }
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            EtVal::String(s) => s.clone(),
            EtVal::Array(vec) => vec
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

#[derive(Debug)]
pub struct ExiftoolEntry {
    pub file_name: PathBuf,
    pub tag_entries: Vec<TagEntry>,
}

impl ExiftoolEntry {
    pub fn as_hashmap(&self) -> HashMap<TagEntryKey, &TagEntry> {
        self.tag_entries.iter().map(|e| (e.as_key(), e)).collect()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TagEntry {
    #[serde(skip)]
    pub short_name: String,
    #[serde(skip)]
    pub instance: String,
    #[serde(skip)]
    pub binary_size_kb: Option<f32>,
    #[serde(rename = "desc")]
    pub name: String,
    #[serde(deserialize_with = "deserialize_id")]
    pub id: Option<u64>,
    #[serde(deserialize_with = "deserialize_table")]
    pub table: (String, String),
    pub val: EtVal,
    pub num: Option<EtVal>,
    pub index: Option<u64>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct TagEntryKey {
    pub short_name: String,
    pub table: (String, String),
}

// impl From<TagEntry> for TagEntryKey {
//     fn from(value: TagEntry) -> Self {
//         Self {
//             short_name: value.short_name.clone(),
//             table: value.table.clone(),
//         }
//     }
// }

fn deserialize_id<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let deserialized = u64::deserialize(deserializer);
    Ok(deserialized.map_or(None, |num| Some(num)))
}

fn deserialize_table<'de, D>(deserializer: D) -> Result<(String, String), D::Error>
where
    D: Deserializer<'de>,
{
    let deserialized = String::deserialize(deserializer)?;
    if let Some(sep_pos) = deserialized.find("::") {
        Ok((
            deserialized[..sep_pos].to_owned(),
            deserialized[sep_pos + 2..].to_owned(),
        ))
    } else {
        Ok((deserialized, String::new()))
    }
}

impl PartialEq for TagEntry {
    fn eq(&self, other: &Self) -> bool {
        self.short_name == other.short_name
            && self.binary_size_kb == other.binary_size_kb
            && self.id == other.id
            && self.table == other.table
            && self.val == other.val
            && self.index == other.index
    }
}

// impl Eq for TagEntry {}

impl TagEntry {
    pub fn check_filter(&self, filter: &str) -> bool {
        let filter = filter.to_lowercase();
        if filter.starts_with("<<") && filter.ends_with(">>") {
            self.table_to_string()
                .to_lowercase()
                .contains(&filter[2..filter.len() - 2])
        } else {
            self.name.to_lowercase().contains(&filter)
                || self.short_name.to_lowercase().contains(&filter)
                || self.val.check_filter(&filter)
                || self
                    .num
                    .as_ref()
                    .is_some_and(|num| num.check_filter(&filter))
        }
    }

    pub fn open_web_page(&self) {
        if self.table.0 == "Exif" {
            let _ = open::that("https://exiftool.org/TagNames/EXIF.html");
            return;
        }
        let _ = open::that(format!(
            "https://exiftool.org/TagNames/{}.html",
            self.table.0
        ));
    }

    pub fn table_to_string(&self) -> String {
        if self.table.1.is_empty() {
            self.table.0.clone()
        } else {
            format!("{}::{}", self.table.0, self.table.1)
        }
    }

    pub fn to_string(&self) -> String {
        let mut res = format!(
            "Name: {}
Short name: {}
Tag ID: {}
Tag family: {}
Tag value: {}
Tag numerical value: {}",
            self.name,
            self.short_name,
            if let Some(id) = self.id {
                format!("{} ({:#X})", id, id)
            } else {
                "Unknown".into()
            },
            self.table_to_string(),
            self.val.to_string(),
            if let Some(num) = &self.num {
                num.to_string()
            } else {
                self.val.to_string()
            }
        );

        if let Some(index) = self.index {
            res.push_str("\nTag index: ");
            res += &index.to_string();
        }

        res
    }

    pub fn get_binary(&self, image_path: &Path) -> Result<Vec<u8>, ()> {
        if let None = self.binary_size_kb {
            return Err(());
        }

        Ok(Command::new("exiftool")
            .arg(image_path)
            .arg(&format!("-{}", self.short_name))
            .arg("-b")
            .output()
            .map_err(|_| ())?
            .stdout)
    }

    pub fn as_key(&self) -> TagEntryKey {
        TagEntryKey {
            short_name: self.short_name.clone(),
            table: self.table.clone(),
        }
    }
}

// fn parse_a(a: &str, entry: &mut ExiftoolEntry) {
//     let group_end_pos = if a[0..1].contains("[") {
//         let group_end_pos = a.find("]").unwrap();
//         let group_path = &a[1..group_end_pos];
//         entry.tag_path = group_path.split("-").map(|s| s.to_owned()).collect();
//         group_end_pos
//     } else {
//         0
//     };
//     let a_main = a[group_end_pos + 1..].trim();
//     let sep_pos = a_main.find(":").unwrap();
//     entry.name = a_main[..sep_pos].trim().to_owned();
//     if sep_pos + 1 >= a_main.len() {
//         return; // no value
//     }
//     let val_trimmed = a_main[sep_pos + 1..].trim();
//     if val_trimmed.contains("bytes") {
//         let num_bytes: f32 = val_trimmed
//             .chars()
//             .filter(|ch| ch.is_ascii_digit())
//             .collect::<String>()
//             .parse()
//             .unwrap();
//         entry.binary_size_kb = Some(num_bytes / 1024f32);
//     } else {
//         entry.value = val_trimmed.to_owned();
//     }
// }

// fn parse_b(b: &str, entry: &mut ExiftoolEntry) {
//     let group_end_pos = if b[0..1].contains("[") {
//         let group_end_pos = b.find("]").unwrap();
//         let group_path = &b[1..group_end_pos];
//         entry.tag_path = group_path.split("-").map(|s| s.to_owned()).collect();
//         group_end_pos
//     } else {
//         0
//     };
//     let b_plus = b[group_end_pos + 1..].trim();
//     let main_begin_pos = if !b_plus.starts_with("-") {
//         let space_pos = b_plus.find(" ").unwrap();
//         entry.tag_dec = Some(b_plus[..space_pos].parse().unwrap());
//         space_pos + 1
//     } else {
//         2
//     };
//     let b_main = b_plus[main_begin_pos..].trim();
//     let sep_pos = b_main.find(":").unwrap();
//     entry.short_name = b_main[..sep_pos].to_owned();
//     if entry.binary_size_kb.is_none() {
//         if sep_pos + 1 >= b_main.len() {
//             return; // no value
//         }
//         entry.numerical_value = b_main[sep_pos + 1..].to_owned();
//     }
// }

fn read_entry(from: &mut Value) -> ExiftoolEntry {
    let mut res = ExiftoolEntry {
        file_name: PathBuf::new(),
        tag_entries: vec![],
    };
    for (k, v) in from.as_object_mut().unwrap() {
        if let Value::String(s) = v {
            if k.contains("SourceFile") {
                res.file_name = PathBuf::from(&s);
            }
        }
        if !matches!(&v, Value::Object(_)) {
            continue;
        }
        if let Value::Number(num) = &v["num"] {
            v["num"] = Value::String(num.to_string());
        }
        if let Value::Number(num) = &v["val"] {
            v["val"] = Value::String(num.to_string());
        }
        if let Value::Bool(num) = &v["num"] {
            v["num"] = Value::String(num.to_string());
        }
        if let Value::Bool(num) = &v["val"] {
            v["val"] = Value::String(num.to_string());
        }
        let mut entry: TagEntry = serde_json::from_value(v.clone()).unwrap();
        if let Some(sep_pos) = k.find(":") {
            entry.instance = k[..sep_pos].to_owned();
            entry.short_name = k[sep_pos + 1..].to_owned();
        } else {
            entry.short_name = k.clone();
        }
        if let EtVal::String(s) = &entry.val {
            if s.contains("bytes") {
                let num_bytes: f32 = s
                    .chars()
                    .filter(|ch| ch.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap();
                entry.binary_size_kb = Some(num_bytes / 1024f32);
            }
        }
        res.tag_entries.push(entry);
    }
    res
}

pub fn run(input: Vec<PathBuf>, recursive: bool) -> std::io::Result<Vec<ExiftoolEntry>> {
    let mut et_cmd = Command::new("exiftool");
    et_cmd
        .args(input)
        .arg("-j")
        .arg("-G4")
        .arg("-l")
        .arg("-D")
        .arg("-t");
    if recursive {
        et_cmd.arg("-r");
    }
    let et_out = et_cmd.output().unwrap();

    let mut res = Vec::new();
    let mut sval: Value = serde_json::from_slice(&et_out.stdout).unwrap();
    for file_out in sval.as_array_mut().unwrap() {
        res.push(read_entry(file_out));
    }

    Ok(res)
}

// #[test]
// fn t() {
//     let image_path = "/Users/mikhailmatsykh/Downloads/2024-09-06 175947.dng";

//     let et_out_a = Command::new("exiftool").arg(image_path).output().unwrap();
//     let et_out_b = Command::new("exiftool")
//         .arg("-s2")
//         .arg("-n")
//         .arg("-D")
//         .arg("-G")
//         .arg(image_path)
//         .output()
//         .unwrap();

//     let et_out_a =
//         String::from_utf8(et_out_a.stdout).expect("Could not convert exiftool output to string");
//     let et_out_b =
//         String::from_utf8(et_out_b.stdout).expect("Could not convert exiftool output to string");

//     let mut res = Vec::new();
//     for (out, sr_out) in et_out_a.split("\n").zip(et_out_b.split("\n")) {
//         if out.contains(":") {
//             let mut entry = ExiftoolEntry::default();
//             println!("\na: {}", out);
//             parse_a(out, &mut entry);
//             println!("b: {}", sr_out);
//             parse_b(sr_out, &mut entry);
//             res.push(entry);
//         }
//     }
// }

#[test]
fn t_serde() {
    let image_path = "/Users/mikhailmatsykh/Downloads/2024-09-06 175947.dng";

    let et_out = Command::new("exiftool")
        .arg(image_path)
        .arg("-j")
        .arg("-G4")
        .arg("-l")
        .arg("-D")
        .arg("-t")
        .output()
        .unwrap();

    let mut sval: Value = serde_json::from_slice(&et_out.stdout).unwrap();
    for (k, v) in sval[0].as_object_mut().unwrap() {
        if !matches!(&v, Value::Object(_)) {
            continue;
        }
        if let Value::Number(num) = &v["num"] {
            v["num"] = Value::String(num.to_string());
        }
        if let Value::Number(num) = &v["val"] {
            v["val"] = Value::String(num.to_string());
        }
        if let Value::Bool(num) = &v["num"] {
            v["num"] = Value::String(num.to_string());
        }
        if let Value::Bool(num) = &v["val"] {
            v["val"] = Value::String(num.to_string());
        }
        println!("\n\n{:?}", v);
        let mut entry: TagEntry = serde_json::from_value(v.clone()).unwrap();
        if let Some(sep_pos) = k.find(":") {
            entry.instance = k[..sep_pos].to_owned();
            entry.short_name = k[sep_pos + 1..].to_owned();
        } else {
            entry.short_name = k.clone();
        }
        if let EtVal::String(s) = &entry.val {
            if s.contains("bytes") {
                let num_bytes: f32 = s
                    .chars()
                    .filter(|ch| ch.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap();
                entry.binary_size_kb = Some(num_bytes / 1024f32);
            }
        }
        println!("\n{:?}", entry);
    }
}
