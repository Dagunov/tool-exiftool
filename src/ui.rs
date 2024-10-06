use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        block::Title, Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};

use crate::app::{et_wrapper::TagEntry, App, BinarySaveDialog, MainInput, MainState, Screen};

pub fn ui(frame: &mut Frame, app: &mut App) {
    let outer_layout =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(4)]).split(frame.area());

    match &app.screen {
        Screen::Main(input) => {
            let outer_layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
                .split(outer_layout[0]);
            draw_filename(frame, app, outer_layout[0]);
            let mut main_layout = outer_layout[1];
            if app.main_state.is_multiple_files() && app.main_state.compare_data.mode.is_none() {
                let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
                    .split(main_layout);
                draw_tabs(frame, &app.main_state, layout[0]);
                main_layout = layout[1];
            }
            if app.main_state.show_details {
                let layout = Layout::horizontal([
                    Constraint::Fill(if app.main_state.compare_data.mode.is_some() {
                        3
                    } else {
                        2
                    }),
                    Constraint::Fill(1),
                ])
                .split(main_layout);
                draw_details(frame, &mut app.main_state, layout[1]);
                main_layout = layout[0];
            }
            if !app.main_state.filter.is_empty() || matches!(input, MainInput::Filter) {
                let layout = Layout::vertical([Constraint::Length(2), Constraint::Fill(1)])
                    .split(main_layout);
                draw_filter(frame, &mut app.main_state, layout[0]);
                main_layout = layout[1];
            }
            if app.main_state.is_multiple_files() && app.main_state.compare_data.mode.is_some() {
                draw_main_compare(frame, &mut app.main_state, main_layout);
            } else {
                draw_main(frame, &mut app.main_state, main_layout);
            }
            if let Some(dialog) = &mut app.main_state.binary_save_dialog {
                let popup_layout = centered_rect(60, 8, frame.area());
                draw_binary_save_dialog(frame, dialog, popup_layout);
            }
        }
        Screen::Help => draw_help(frame, outer_layout[0]),
        Screen::MiltipleFilesStart => draw_multiple_files_start(frame, outer_layout[0]),
    }

    draw_hints(frame, app, outer_layout[1]);
}

fn draw_filter(frame: &mut Frame, state: &mut MainState, layout: Rect) {
    let filter_block = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .bold()
        .title(" Filter ");
    let par = Paragraph::new(state.filter.as_str()).block(filter_block);
    frame.render_widget(par, layout);
}

fn draw_main(frame: &mut Frame, state: &mut MainState, layout: Rect) {
    let inner_layout =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).split(layout);

    let mut key_lines = vec![];
    let mut val_lines = vec![];
    for (i, entry) in state.et_data[state.current_file_index]
        .tag_entries
        .iter()
        .filter(|ee| state.filter.is_empty() || ee.check_filter(&state.filter))
        .enumerate()
    {
        let mut style = if entry.short_name.to_lowercase().contains("warning") {
            Style::default().fg(Color::LightYellow)
        } else if entry.short_name.to_lowercase().contains("error") {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        let key_str = if state.data_display_mode.short {
            entry.short_name.clone()
        } else {
            entry.name.clone()
        };

        let val_str = if let Some(kb_size) = entry.binary_size_kb {
            style = style.fg(Color::LightGreen);
            format!("{:.1}Kb binary data; Can be extracted", kb_size)
        } else {
            let num = &entry.num;
            if state.data_display_mode.numerical && num.is_some() {
                num.as_ref().unwrap().to_string()
            } else {
                entry.val.to_string()
            }
        };
        if i == state.cursor {
            style = style.patch(Style::default().black().on_white().bold());
        }

        key_lines.push(
            Line::from(cut_string(key_str, &inner_layout[0], state.scroll_offset.1)).style(style),
        );
        val_lines.push(
            Line::from(cut_string(val_str, &inner_layout[1], state.scroll_offset.1)).style(style),
        );
    }
    state.num_entries_shown = key_lines.len();
    let num_entries_in_viewport = layout.height.saturating_sub(2) as usize;
    let need_scrollbar = num_entries_in_viewport < state.num_entries_shown;

    if state.cursor < state.scroll_offset.0 as usize {
        state.scroll_offset.0 = state.cursor as u16;
    } else if state.cursor >= state.scroll_offset.0 as usize + num_entries_in_viewport {
        state.scroll_offset.0 = (state.cursor - num_entries_in_viewport + 1) as u16;
    }
    state.scroll_offset.0 = state
        .scroll_offset
        .0
        .min(state.num_entries_shown.saturating_sub(5) as u16);

    let key_block = Block::bordered().title(
        if state.data_display_mode.short {
            " Tag [Short] "
        } else {
            " Tag [Detailed] "
        }
        .bold(),
    );
    let val_block = Block::default()
        .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
        .title(
            if state.data_display_mode.numerical {
                " Value [Numerical] "
            } else {
                " Value [Readable] "
            }
            .bold(),
        );

    let key_par = Paragraph::new(key_lines)
        .scroll(state.scroll_offset)
        .block(key_block);
    let val_par = Paragraph::new(val_lines)
        .scroll(state.scroll_offset)
        .block(val_block);

    frame.render_widget(key_par, inner_layout[0]);
    frame.render_widget(val_par, inner_layout[1]);

    if need_scrollbar {
        let mut sb_state = ScrollbarState::new(state.num_entries_shown).position(state.cursor);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalLeft)
            .track_style(Style::default().fg(Color::Blue))
            .thumb_style(Style::default().fg(Color::LightBlue));
        frame.render_stateful_widget(sb, inner_layout[0], &mut sb_state);
    }
}

fn transpose2<T>(v: Vec<Vec<T>>) -> Vec<Vec<T>> {
    assert!(!v.is_empty());
    let len = v[0].len();
    let mut iters: Vec<_> = v.into_iter().map(|n| n.into_iter()).collect();
    (0..len)
        .map(|_| {
            iters
                .iter_mut()
                .map(|n| n.next().unwrap())
                .collect::<Vec<T>>()
        })
        .collect()
}

fn draw_main_compare(frame: &mut Frame, state: &mut MainState, layout: Rect) {
    let only_diff = state.compare_data.mode.unwrap();
    let small_parts_num = 1 + state.et_data.len() as u32 * 2;
    let mut constraints = vec![Constraint::Ratio(1, small_parts_num)];
    for _ in 0..state.et_data.len() {
        constraints.push(Constraint::Ratio(2, small_parts_num));
    }
    let inner_layout = Layout::horizontal(constraints).split(layout);
    let mut key_lines = vec![];
    let mut val_lines = vec![];

    let check_filter = |v: &Vec<Option<TagEntry>>| {
        state.filter.is_empty()
            || v.iter()
                .any(|v| v.as_ref().is_some_and(|v| v.check_filter(&state.filter)))
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

    for (i, (k, vals)) in state
        .compare_data
        .data
        .iter()
        .filter(|(_, v)| check_filter(v) && check_diff(v))
        .enumerate()
    {
        let mut style = if k.short_name.to_lowercase().contains("warning") {
            Style::default().fg(Color::LightYellow)
        } else if k.short_name.to_lowercase().contains("error") {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        let key_str = if state.data_display_mode.short {
            k.short_name.clone()
        } else {
            k.name.clone()
        };

        let val_strs = vals
            .iter()
            .map(|v| {
                if let Some(v) = v {
                    if let Some(kb_size) = v.binary_size_kb {
                        style = style.fg(Color::LightGreen);
                        format!("{:.1}Kb binary data; Can be extracted", kb_size)
                    } else {
                        let num = &v.num;
                        if state.data_display_mode.numerical && num.is_some() {
                            num.as_ref().unwrap().to_string()
                        } else {
                            v.val.to_string()
                        }
                    }
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>();

        if i == state.cursor {
            style = style.patch(Style::default().black().on_white().bold());
        }

        key_lines.push(
            Line::from(cut_string(key_str, &inner_layout[0], state.scroll_offset.1)).style(style),
        );
        val_lines.push(
            val_strs
                .into_iter()
                .map(|v| {
                    Line::from(cut_string(v, &inner_layout[1], state.scroll_offset.1)).style(style)
                })
                .collect::<Vec<_>>(),
        );
    }
    state.num_entries_shown = key_lines.len();
    let num_entries_in_viewport = layout.height.saturating_sub(2) as usize;
    let need_scrollbar = num_entries_in_viewport < state.num_entries_shown;

    if state.cursor < state.scroll_offset.0 as usize {
        state.scroll_offset.0 = state.cursor as u16;
    } else if state.cursor >= state.scroll_offset.0 as usize + num_entries_in_viewport {
        state.scroll_offset.0 = (state.cursor - num_entries_in_viewport + 1) as u16;
    }
    state.scroll_offset.0 = state
        .scroll_offset
        .0
        .min(state.num_entries_shown.saturating_sub(5) as u16);

    let key_block = Block::bordered().title(
        if state.data_display_mode.short {
            " Tag [Short] "
        } else {
            " Tag [Detailed] "
        }
        .bold(),
    );
    let val_blocks = state
        .et_data
        .iter()
        .enumerate()
        .map(|(col, et)| {
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                .title(
                    if state.data_display_mode.numerical {
                        " Value [Numerical] "
                    } else {
                        " Value [Readable] "
                    }
                    .bold(),
                )
                .title_bottom({
                    let title_str = et.file_name.to_str().unwrap_or("[INVALID FILE NAME]");
                    let mut res = if inner_layout[col + 1].width as usize + 2 >= title_str.len() {
                        title_str.to_owned()
                    } else {
                        format!(
                            "*{}",
                            &title_str
                                [title_str.len() - inner_layout[col + 1].width as usize + 2..]
                        )
                    }
                    .bold();
                    if col == state.current_file_index {
                        res = res.on_green().black();
                    }
                    res
                })
        })
        .collect::<Vec<_>>();

    let key_par = Paragraph::new(key_lines)
        .scroll(state.scroll_offset)
        .block(key_block);

    let val_lines = transpose2(val_lines);

    let val_pars = val_lines
        .into_iter()
        .zip(val_blocks)
        .map(|(l, b)| Paragraph::new(l).scroll(state.scroll_offset).block(b));

    frame.render_widget(key_par, inner_layout[0]);

    for (i, par) in val_pars.enumerate() {
        frame.render_widget(par, inner_layout[i + 1]);
    }

    if need_scrollbar {
        let mut sb_state = ScrollbarState::new(state.num_entries_shown).position(state.cursor);
        let sb = Scrollbar::new(ScrollbarOrientation::VerticalLeft)
            .track_style(Style::default().fg(Color::Blue))
            .thumb_style(Style::default().fg(Color::LightBlue));
        frame.render_stateful_widget(sb, inner_layout[0], &mut sb_state);
    }
}

fn draw_filename(frame: &mut Frame, app: &App, layout: Rect) {
    let title = if app.main_state.compare_data.mode.is_some() {
        "Compare Mode".to_owned()
    } else if let Some(file_name) = app.main_state.current_file.to_str() {
        if file_name.len() >= layout.width.saturating_sub(2) as usize {
            "...".to_owned()
                + &file_name[file_name
                    .len()
                    .saturating_sub(layout.width.saturating_sub(2) as usize)
                    + 3..]
        } else {
            file_name.to_owned()
        }
    } else {
        "[INVALID FILE NAME]".to_owned()
    };
    let block = Block::bordered().title(title).bold().black().on_white();
    frame.render_widget(block, layout);
}

fn draw_hints(frame: &mut Frame, app: &mut App, layout: Rect) {
    let hint_block = Block::bordered();

    let hint_lines = if let Some(log_msg) = app.main_state.log_msg.take() {
        match log_msg {
            Ok(msg) => vec![Line::from(msg).green()],
            Err(msg) => vec![Line::from(msg).red()],
        }
    } else {
        match &app.screen {
            Screen::Main(input) if matches!(input, MainInput::Main) => {
                vec![
                    Line::from("<↑/↓/←/→/WHEEL> - scroll  <f> - filter  <ENTER> - details"),
                    Line::from(vec!["<h> - help  ".light_yellow(), "<q> - quit".red()]),
                ]
            }
            Screen::Main(input) if matches!(input, MainInput::Filter) => {
                vec![
                    Line::from("Filtering by tags and values.".cyan()),
                    Line::from(vec!["<ENTER> - apply  ".green(), "<ESC> - discard".red()]),
                ]
            }
            Screen::Help => {
                vec![Line::from("<ENTER/ESC/q> - go back")]
            }
            Screen::MiltipleFilesStart => {
                vec![Line::from("<q> - quit")]
            }
            _ => vec![],
        }
    };
    let par = Paragraph::new(hint_lines)
        .block(hint_block)
        .wrap(Wrap::default());

    frame.render_widget(par, layout);
}

fn draw_details(frame: &mut Frame, state: &MainState, layout: Rect) {
    if let Some(entry) = state.selected_entry() {
        let block = Block::default()
            .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
            .title((" Details [".to_owned() + &entry.short_name + "] ").bold());

        let mut data = vec![
            Line::from("Detailed name: ".to_owned() + &entry.name),
            Line::from(
                "Tag ID: ".to_owned()
                    + &if let Some(id) = entry.id {
                        id.to_string() + &format!(" ({:#X})", id)
                    } else {
                        "[Unknown]".to_owned()
                    },
            ),
            Line::from(vec![
                Span::from("Tag family: "),
                entry.table_to_string().into(),
                " <F> - filter by tag family".yellow(),
            ]),
            Line::from({
                let strval = entry.val.to_string();
                if strval.len() > layout.width as usize * 5 {
                    vec![
                        Span::from("Value: "),
                        strval.as_str()[..layout.width as usize * 3]
                            .to_owned()
                            .into(),
                        "... value too long, press <x> to copy".yellow(),
                    ]
                } else {
                    vec![Span::from("Value: "), strval.into()]
                }
            }),
            Line::from({
                let strval = if let Some(num) = &entry.num {
                    num.to_string()
                } else {
                    entry.val.to_string()
                };
                if strval.len() > layout.width as usize * 5 {
                    vec![
                        Span::from("Numerical value: "),
                        strval.as_str()[..layout.width as usize * 3]
                            .to_owned()
                            .into(),
                        "... value too long, press <X> to copy".yellow(),
                    ]
                } else {
                    vec![Span::from("Numerical value: "), strval.into()]
                }
            }),
        ];

        if let Some(index) = &entry.index {
            data.push(Line::from(format!("Index: {index}")));
        }

        data.push(Line::default());
        data.push(Line::from("<C> - copy entry to clipboard").yellow());
        if let Some(_) = entry.binary_size_kb {
            data.push(Line::from("<b> - extract binary data").yellow());
        }

        let par = Paragraph::new(data).block(block).wrap(Wrap::default());

        frame.render_widget(par, layout);
    }
}

/// |- Main Title ------------|
/// |- Fname ---------- Fext -|
/// |            |            |
/// |-------------------------|
/// | saving to ...           |
/// | jpeg                    |
/// | controls                |
/// |-------------------------|
fn draw_binary_save_dialog(frame: &mut Frame, state: &mut BinarySaveDialog, layout: Rect) {
    let bg_block = Block::default().on_dark_gray();
    frame.render_widget(Clear, layout);
    frame.render_widget(bg_block, layout);

    let layout = Layout::vertical([Constraint::Length(4), Constraint::Length(4)]).split(layout);

    let main_block = Block::bordered().title(
        Title::from(" Save binary data ".bold()).alignment(ratatui::layout::Alignment::Center),
    );

    let main_subblock = Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
        .title(" File name ")
        .title(Title::from(" Extension ").alignment(ratatui::layout::Alignment::Right));

    let mut subblock_layout = layout[0];
    subblock_layout.height = subblock_layout.height.saturating_sub(1);
    subblock_layout.y += 1;

    let mut input_layout = subblock_layout;
    input_layout.height = 1;
    input_layout.y += 1;
    input_layout.width = input_layout.width.saturating_sub(2);
    input_layout.x += 1;

    let fname_block = Block::default().borders(Borders::RIGHT);

    let mut fname_spans: Vec<Span> = vec![state.fname.as_str().into()];
    let mut fext_spans: Vec<Span> = vec![state.fext.as_str().into()];
    if state.editing_fname {
        fname_spans.push(" ".on_white());
    } else {
        fext_spans.push(" ".on_white());
    }

    let fname_par = Paragraph::new(Line::from(fname_spans)).block(fname_block);
    let fext_line = Line::from(fext_spans);

    let input_layout =
        Layout::horizontal([Constraint::Fill(1), Constraint::Percentage(20)]).split(input_layout);

    frame.render_widget(main_block, layout[0]);
    frame.render_widget(main_subblock, subblock_layout);
    frame.render_widget(fname_par, input_layout[0]);
    frame.render_widget(fext_line, input_layout[1]);

    let bot_block = Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM);

    let bot_text = Text::from(vec![
        match &state.status {
            Ok(msg) => Line::from(msg.as_str()),
            Err(msg) => Line::from(msg.as_str()).red(),
        },
        Line::from(vec![
            "<ENTER> - save ".green(),
            "<ESC> - discard ".red(),
            "<TAB> - switch focus".into(),
        ]),
    ]);

    let bot_par = Paragraph::new(bot_text)
        .block(bot_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(bot_par, layout[1]);
}

fn draw_help(frame: &mut Frame, layout: Rect) {
    let block = Block::bordered().title("Help");

    let lines = vec![
        Line::from("General controls").bold().centered(),
        Line::from("<↑/↓/←/→/WHEEL/SPACE> - scroll      <f> - filter by tags/values"),
        Line::from("<ENTER> - toggle show details       <s> - toggle show short tag names"),
        Line::from("<n> - toggle show numerical representation of tag values"),
        Line::from("<b> - save binary data from tag     <h> - show this text"),
        Line::from("<q> - quit"),
        Line::default(),
        Line::from("Extra controls").bold().centered(),
        Line::from(
            "<x> - copy tag value to clipboard   <X> - copy tag numerical value to clipboard",
        ),
        Line::from("<C> - copy all entry data to clipboard"),
        Line::from("<F> - filter by current tag's group (family)"),
        Line::from("<w> - try to open a web page with this tag's family's information"),
        Line::default(),
        Line::from("Multiple files extra controls").bold().centered(),
        Line::from("<TAB> - next tab                    <SHIFT+TAB> - previous tab"),
        Line::from("<c> - toggle side-by-side compare mode"),
        Line::from("<d> - while in side-by-side compare mode, show only lines that differ"),
        Line::default(),
        Line::from("You can still change tabs while in side-by-side compare mode;"),
        Line::from("this will control what details will be shown, what data will be copied, extracted etc."),
    ];

    let par = Paragraph::new(lines).block(block);

    frame.render_widget(par, layout);
}

fn draw_multiple_files_start(frame: &mut Frame, layout: Rect) {
    let main_line = Line::from("You provided one or more folders as input. Please choose if you want to read them recursively:").bold().centered();
    let main_par = Paragraph::new(main_line).wrap(Wrap::default());

    let vertical_layout = Layout::vertical([
        Constraint::Max(5),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Percentage(40),
    ])
    .flex(ratatui::layout::Flex::SpaceAround)
    .split(layout);

    frame.render_widget(main_par, vertical_layout[0]);
    frame.render_widget(
        Line::from("<y/ENTER>   YES").on_green().bold().centered(),
        vertical_layout[1],
    );
    frame.render_widget(
        Line::from("<n/ESC>     NO").on_red().bold().centered(),
        vertical_layout[2],
    );
}

fn draw_tabs(frame: &mut Frame, state: &MainState, layout: Rect) {
    let num_files = state.et_data.len();
    let tab_len = (layout.width as f32 * 0.95 / num_files as f32) as u16;
    if tab_len < 6 {
        frame.render_widget(
            Line::from("Too many files to show tabs, <TAB> can still be used").yellow(),
            layout,
        );
        return;
    }
    let tab_layout = Layout::horizontal(Constraint::from_lengths([tab_len].repeat(num_files)))
        .flex(ratatui::layout::Flex::Start)
        .split(layout);

    let tab_width = tab_layout[0].width;
    let take_text = tab_width.saturating_sub(4) as usize;

    for (i, et) in state.et_data.iter().enumerate() {
        let fname = et
            .file_name
            .to_str()
            .expect("File path contains bad unicode");
        let text = &fname[fname.len().saturating_sub(take_text + 1)..];
        let mut line = Line::from(vec![
            "|".red().bold(),
            "*".yellow(),
            text.into(),
            "|".red().bold(),
        ]);
        if i == state.current_file_index {
            line = line.bold().white().on_dark_gray();
        }
        frame.render_widget(line, tab_layout[i]);
    }
}

fn cut_string(mut s: String, target: &Rect, x_offset: u16) -> String {
    if x_offset as usize >= s.len() && !s.is_empty() {
        return ".".repeat((x_offset + 3) as usize).to_owned();
    }
    if s.len().saturating_sub(x_offset as usize) >= (target.width - 2) as usize {
        s.truncate((x_offset + target.width.saturating_sub(5)) as usize);
        s += "...";
    }
    if x_offset != 0 {
        let mid = (x_offset + 3) as usize;
        if mid >= s.len() {
            s = ".".repeat(mid);
        } else {
            s = ".".repeat(x_offset as usize + 3) + s.split_at(mid).1;
        }
    }
    s
}

fn centered_rect(percent_x: u16, size_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(size_y),
        Constraint::Fill(1),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

#[test]
fn cut_test() {
    let s = String::from("1234567890123");
    let s = cut_string(s, &Rect::new(0, 0, 12, 7), 1);
    println!("s: {s}");
}
