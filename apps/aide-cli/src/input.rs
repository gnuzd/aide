use crate::theme::{Theme, all_themes, get_theme};
use aide_core::Aide;
use crossterm::style::{SetBackgroundColor, SetForegroundColor, ResetColor, Attribute, SetAttribute};
use std::io::{Write, stdout};

const CMDS: &[(&str, &str)] = &[
    ("/bug", "report an issue with logs"),
    (
        "/clear",
        "clear data (conversations, profile, models, config)",
    ),
    ("/exit", "exit the application"),
    ("/help", "show this help"),
    ("/memory", "show what Aide currently knows about you"),
    ("/models", "list available models"),
    ("/system", "show system information"),
    ("/theme", "list or switch color themes"),
];

fn prev_char(pos: usize, s: &str) -> usize {
    if pos == 0 { return 0; }
    let mut i = pos - 1;
    while i > 0 && !s.is_char_boundary(i) { i -= 1; }
    i
}

fn next_char(pos: usize, s: &str) -> usize {
    if pos >= s.len() { return s.len(); }
    let mut i = pos + 1;
    while i < s.len() && !s.is_char_boundary(i) { i += 1; }
    i
}

pub fn read_chat_line(aide: &Aide) -> anyhow::Result<Option<String>> {
    crossterm::terminal::enable_raw_mode()?;
    let result = read_chat_line_inner(aide);
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

fn read_chat_line_inner(aide: &Aide) -> anyhow::Result<Option<String>> {
    use crossterm::cursor::{MoveToColumn, MoveUp};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use crossterm::terminal::{Clear, ClearType};

    let mut custom_themes = Vec::new();
    for v in &aide.config.custom_themes {
        if let Ok(t) = serde_json::from_value::<Theme>(v.clone()) {
            custom_themes.push(t);
        }
    }
    let themes_list = all_themes(&custom_themes);
    let initial_theme_name = aide.config.active_theme.as_deref().unwrap_or("gruvbox").to_string();
    let mut current_theme;

    let mut buf = String::new();
    let mut cursor_pos: usize = 0;
    let mut menu_sel: i32 = -1;
    let mut last_had_menu = false;

    loop {
        let is_theme_submode = buf.starts_with("/theme ");
        let is_clear_submode = buf.starts_with("/clear ");
        let show_menu = buf.starts_with('/');
        
        let (menu_items, filtered): (Vec<(String, String)>, Vec<usize>) = if is_theme_submode {
            let filter_text = &buf[7..];
            let items: Vec<(String, String)> = themes_list.iter().map(|t| (t.name.clone(), "theme".to_string())).collect();
            let filtered_indices: Vec<usize> = items.iter()
                .enumerate()
                .filter(|(_, item)| item.0.starts_with(filter_text))
                .map(|(i, _)| i)
                .collect();
            (items, filtered_indices)
        } else if is_clear_submode {
            let filter_text = &buf[7..];
            let items: Vec<(String, String)> = vec![
                ("all", "factory reset (EVERYTHING)").into(),
                ("chat", "clear conversation history").into(),
                ("profile", "clear memory/profile").into(),
                ("models", "delete all downloaded models").into(),
                ("config", "reset settings to default").into(),
            ].into_iter().map(|(a, d): (&str, &str)| (a.to_string(), d.to_string())).collect();
            let filtered_indices: Vec<usize> = items.iter()
                .enumerate()
                .filter(|(_, item)| item.0.starts_with(filter_text))
                .map(|(i, _)| i)
                .collect();
            (items, filtered_indices)
        } else if show_menu {
            let items: Vec<(String, String)> = CMDS.iter().map(|(c, d)| (c.to_string(), d.to_string())).collect();
            let filtered_indices: Vec<usize> = items.iter()
                .enumerate()
                .filter(|(_, item)| item.0.starts_with(&buf))
                .map(|(i, _)| i)
                .collect();
            (items, filtered_indices)
        } else {
            (vec![], vec![])
        };

        if filtered.is_empty() {
            menu_sel = -1;
        } else {
            menu_sel = menu_sel.max(0).min(filtered.len() as i32 - 1);
        }

        if is_theme_submode && menu_sel >= 0 {
            let theme_idx = filtered[menu_sel as usize];
            current_theme = themes_list[theme_idx].clone();
        } else {
            current_theme = get_theme(&initial_theme_name, &custom_themes);
        }

        let prompt_sep = "User: ";
        let prompt_cols: u16 = prompt_sep.len() as u16;
        let menu_rows = filtered.len() as u16;

        // REDRAW
        crossterm::execute!(stdout(), MoveToColumn(0))?;
        if last_had_menu {
            crossterm::execute!(stdout(), Clear(ClearType::FromCursorDown))?;
        } else {
            crossterm::execute!(stdout(), Clear(ClearType::CurrentLine))?;
        }
        
        crossterm::execute!(stdout(), SetBackgroundColor(current_theme.user_bg_color()), Clear(ClearType::CurrentLine))?;
        crossterm::execute!(stdout(), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold))?;
        print!("{}", prompt_sep);
        crossterm::execute!(stdout(), SetForegroundColor(current_theme.user_fg_color()), SetAttribute(Attribute::Reset), SetBackgroundColor(current_theme.user_bg_color()))?;
        print!("{}", buf);
        crossterm::execute!(stdout(), Clear(ClearType::UntilNewLine))?;

        if show_menu && !filtered.is_empty() {
            for (menu_i, &idx) in filtered.iter().enumerate() {
                let (name, desc) = &menu_items[idx];
                if menu_sel == menu_i as i32 {
                    print!("\r\n  \x1b[7m{:<15}  {}\x1b[0m", name, desc);
                } else {
                    print!("\r\n  \x1b[2m{:<15}  {}\x1b[0m", name, desc);
                }
            }
            let vis_cursor = buf[..cursor_pos].chars().count() as u16;
            crossterm::execute!(stdout(), MoveUp(menu_rows), MoveToColumn(prompt_cols + vis_cursor))?;
        } else {
            let vis_cursor = buf[..cursor_pos].chars().count() as u16;
            crossterm::execute!(stdout(), MoveToColumn(prompt_cols + vis_cursor))?;
        }
        stdout().flush()?;
        last_had_menu = show_menu && !filtered.is_empty();

        let event = crossterm::event::read()?;
        if let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event {
            if kind != KeyEventKind::Press { continue; }
            match code {
                KeyCode::Enter => {
                    if menu_sel >= 0 && !filtered.is_empty() {
                        let selected = &menu_items[filtered[menu_sel as usize]].0;
                        if selected == "/theme" && !is_theme_submode {
                            buf = "/theme ".to_string(); cursor_pos = buf.len(); menu_sel = 0; continue;
                        } else if is_theme_submode {
                            buf = format!("/theme {}", selected);
                        } else {
                            buf = selected.clone();
                        }
                    }
                    if buf.trim() == "/" { buf.clear(); cursor_pos = 0; menu_sel = -1; continue; }
                    
                    crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
                    crossterm::execute!(stdout(), SetBackgroundColor(current_theme.user_bg_color()), Clear(ClearType::CurrentLine))?;
                    crossterm::execute!(stdout(), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold))?;
                    print!("{}", prompt_sep);
                    crossterm::execute!(stdout(), SetForegroundColor(current_theme.user_fg_color()), SetAttribute(Attribute::Reset), SetBackgroundColor(current_theme.user_bg_color()))?;
                    print!("{}", buf);
                    crossterm::execute!(stdout(), Clear(ClearType::UntilNewLine), ResetColor)?;
                    print!("\r\n");
                    stdout().flush()?;
                    return Ok(Some(buf));
                }
                KeyCode::Tab => {
                    if menu_sel >= 0 && !filtered.is_empty() {
                        let selected = &menu_items[filtered[menu_sel as usize]].0;
                        if is_theme_submode { buf = format!("/theme {}", selected); }
                        else if selected == "/theme" { buf = "/theme ".to_string(); }
                        else { buf = selected.clone(); }
                        cursor_pos = buf.len(); menu_sel = 0;
                    }
                }
                KeyCode::Esc => {
                    if show_menu || menu_sel >= 0 { buf.clear(); cursor_pos = 0; menu_sel = -1; }
                    else { return Ok(Some(String::new())); }
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    print!("\r\n"); stdout().flush()?; return Ok(None);
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        menu_sel = if menu_sel <= 0 { filtered.len() as i32 - 1 } else { menu_sel - 1 };
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        menu_sel = if menu_sel >= filtered.len() as i32 - 1 { 0 } else { menu_sel + 1 };
                    }
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char(cursor_pos, &buf);
                        buf.drain(prev..cursor_pos); cursor_pos = prev; menu_sel = -1;
                    }
                }
                KeyCode::Delete => {
                    if cursor_pos < buf.len() {
                        let next = next_char(cursor_pos, &buf);
                        buf.drain(cursor_pos..next); menu_sel = -1;
                    }
                }
                KeyCode::Left => cursor_pos = prev_char(cursor_pos, &buf),
                KeyCode::Right => cursor_pos = next_char(cursor_pos, &buf),
                KeyCode::Home => cursor_pos = 0,
                KeyCode::End => cursor_pos = buf.len(),
                KeyCode::Char(c) => {
                    buf.insert(cursor_pos, c); cursor_pos += c.len_utf8(); menu_sel = -1;
                }
                _ => {}
            }
        }
    }
}
