use std::path::PathBuf;

use app::{App, MainInput, Screen};
use copypasta::ClipboardProvider;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        MouseEvent, MouseEventKind,
    },
    ExecutableCommand,
};
use ratatui::DefaultTerminal;

mod app;
mod ui;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let mut app = if args.len() > 2 {
        App::new_multiple_files(args.skip(1).map(|s| PathBuf::from(s)).collect())
    } else {
        let input_path = PathBuf::from(&args.nth(1).expect("You should provide an image path"));
        if input_path.is_dir() {
            App::new_multiple_files(vec![input_path.to_owned()])
        } else {
            App::new(input_path)
        }
    }?;

    std::io::stdout().execute(EnableMouseCapture).unwrap();

    let mut terminal = ratatui::init();

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        std::io::stdout().execute(DisableMouseCapture).unwrap();
        hook(info);
    }));

    terminal.clear()?;
    run_app(&mut app, terminal)?;
    ratatui::restore();
    std::io::stdout().execute(DisableMouseCapture).unwrap();
    Ok(())
}

fn run_app(app: &mut App, mut terminal: DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| ui::ui(frame, app))?;
        if handle_events(app)? {
            break;
        }
    }
    Ok(())
}

fn handle_events(app: &mut App) -> std::io::Result<bool> {
    match event::read()? {
        Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
            return Ok(handle_key_press_events(key_event, app));
        }
        Event::Mouse(mouse_event) => {
            handle_mouse_event(mouse_event, app);
        }
        _ => {}
    };
    Ok(false)
}

fn handle_mouse_event(mouse_event: MouseEvent, app: &mut App) {
    let state = &mut app.main_state;
    match &app.screen {
        Screen::Main(input) if matches!(input, MainInput::Main) => match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                state.scrollv_drag_cursor(-1);
            }
            MouseEventKind::ScrollDown => {
                state.scrollv_drag_cursor(1);
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_key_press_events(key_event: KeyEvent, app: &mut App) -> bool {
    let state = &mut app.main_state;
    match &mut app.screen {
        Screen::Main(input) if matches!(input, MainInput::Main) => match key_event.code {
            KeyCode::Char(' ') => {
                state.scrollv_drag_cursor(4);
            }
            KeyCode::Char('q') => {
                return true;
            }
            KeyCode::Char('s') => {
                state.data_display_mode.short = !state.data_display_mode.short;
            }
            KeyCode::Char('n') => {
                state.data_display_mode.numerical = !state.data_display_mode.numerical;
            }
            KeyCode::Char('f') => {
                *input = MainInput::Filter;
                state.scroll_offset = (0, 0);
                state.cursor = 0;
            }
            KeyCode::Char('w') => {
                state.selected_entry().inspect(|e| e.open_web_page());
            }
            KeyCode::Char('h') => {
                app.screen = Screen::Help;
            }
            KeyCode::Up => {
                state.scrollv(-1);
            }
            KeyCode::Down => {
                state.scrollv(1);
            }
            KeyCode::Left => {
                state.scrollh(-1);
            }
            KeyCode::Right => {
                state.scrollh(1);
            }
            KeyCode::Enter => {
                state.show_details = !state.show_details;
            }
            KeyCode::Esc if state.show_details => {
                state.show_details = false;
            }
            KeyCode::Char('x') => {
                if let Some(entry) = state.selected_entry() {
                    app.clipboard
                        .set_contents(entry.val.to_string())
                        .expect("Failed to set clipboard contents!");
                    state.log_msg = Some(Ok(String::from("Succesfully copied value to clipboard")));
                }
            }
            KeyCode::Char('X') => {
                if let Some(entry) = state.selected_entry() {
                    app.clipboard
                        .set_contents(if let Some(num) = &entry.num {
                            num.to_string()
                        } else {
                            entry.val.to_string()
                        })
                        .expect("Failed to set clipboard contents!");
                    state.log_msg = Some(Ok(String::from(
                        "Succesfully copied numerical value to clipboard",
                    )));
                }
            }
            KeyCode::Char('C') => {
                if let Some(entry) = state.selected_entry() {
                    app.clipboard
                        .set_contents(entry.to_string())
                        .expect("Failed to set clipboard contents!");
                    state.log_msg = Some(Ok(String::from(
                        "Succesfully copied entry data to clipboard",
                    )));
                }
            }
            KeyCode::Char('b') => {
                if state
                    .selected_entry()
                    .is_some_and(|e| e.binary_size_kb.is_some())
                {
                    state.binary_save_dialog = Some(Default::default());
                    *input = MainInput::BinarySaveDialog;
                } else {
                    state.log_msg = Some(Err(String::from(
                        "Selected entry does not contain any binary data!",
                    )));
                }
            }
            KeyCode::Char('F') => {
                if let Some(entry) = state.selected_entry() {
                    state.filter = format!("<<{}>>", entry.table_to_string());
                    state.scroll_offset = (0, 0);
                    state.cursor = 0;
                }
            }
            KeyCode::Tab if state.is_multiple_files() => {
                state.current_file_index += 1;
                if state.current_file_index >= state.et_data.len() {
                    state.current_file_index = 0;
                }
                state.current_file = state.et_data[state.current_file_index].file_name.clone();
            }
            KeyCode::BackTab if state.is_multiple_files() => {
                if state.current_file_index == 0 {
                    state.current_file_index = state.et_data.len() - 1;
                } else {
                    state.current_file_index -= 1;
                }
                state.current_file = state.et_data[state.current_file_index].file_name.clone();
            }
            KeyCode::Char('W')
                if state.is_multiple_files() && state.compare_data.mode.is_none() =>
            {
                state.et_data.remove(state.current_file_index);
                if state.current_file_index == state.et_data.len() {
                    state.current_file_index -= 1;
                }
                state.current_file = state.et_data[state.current_file_index].file_name.clone();
            }
            KeyCode::Char('c') => {
                if state.compare_data.mode.is_some() {
                    state.compare_data.mode = None;
                } else {
                    state.compare_data.mode = Some(false);
                }
                state.scroll_offset = (0, 0);
                state.cursor = 0;
                state.current_file_index = 0;
            }
            KeyCode::Char('d') if state.compare_data.mode.is_some() => {
                state.compare_data.mode = Some(!state.compare_data.mode.unwrap());
                state.scroll_offset = (0, 0);
                state.cursor = 0;
            }
            _ => {}
        },
        Screen::Main(input) if matches!(input, MainInput::Filter) => match key_event.code {
            KeyCode::Char(ch) => {
                state.filter.push(ch);
            }
            KeyCode::Backspace => {
                state.filter.pop();
            }
            KeyCode::Enter => {
                *input = MainInput::Main;
            }
            KeyCode::Esc => {
                *input = MainInput::Main;
                state.filter.clear();
            }
            _ => {}
        },
        Screen::Main(input) if matches!(input, MainInput::BinarySaveDialog) => match key_event.code
        {
            KeyCode::Char(ch) => {
                if let Some(dialog) = &mut state.binary_save_dialog {
                    if dialog.editing_fname {
                        dialog.fname.push(ch);
                    } else {
                        dialog.fext.push(ch);
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(dialog) = &mut state.binary_save_dialog {
                    if dialog.editing_fname {
                        dialog.fname.pop();
                    } else {
                        dialog.fext.pop();
                    }
                }
            }
            KeyCode::Tab => {
                if let Some(dialog) = &mut state.binary_save_dialog {
                    dialog.editing_fname = !dialog.editing_fname;
                }
            }
            KeyCode::Enter => {
                if let Ok(_) = state.try_save_binary() {
                    state.binary_save_dialog = None;
                    *input = MainInput::Main;
                }
            }
            KeyCode::Esc => {
                *input = MainInput::Main;
                state.binary_save_dialog = None;
            }
            _ => {}
        },
        Screen::Help => match key_event.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.screen = Screen::Main(Default::default());
            }
            _ => {}
        },
        Screen::MiltipleFilesStart => match key_event.code {
            KeyCode::Char('q') => {
                return true;
            }
            KeyCode::Char('y') | KeyCode::Enter => {
                state
                    .read_multiple_files(true)
                    .expect("Failed to read data with exiftool!");
                app.screen = Screen::Main(Default::default());
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                state
                    .read_multiple_files(false)
                    .expect("Failed to read data with exiftool!");
                app.screen = Screen::Main(Default::default());
            }
            _ => {}
        },
        _ => {}
    };
    false
}
