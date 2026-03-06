use crate::theme::{Theme, all_themes, get_theme};
use aide_core::Aide;
use crossterm::style::{SetBackgroundColor, SetForegroundColor, Color, ResetColor, Attribute, SetAttribute};
use std::io::{Write, stdout};

const CMDS: &[(&str, &str)] = &[
    (
        "/clear",
        "clear data (conversations, profile, models, config)",
    ),
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

pub fn read_chat_line(aide: &Aide, history: &[String]) -> anyhow::Result<Option<String>> {
    crossterm::terminal::enable_raw_mode()?;
    let result = read_chat_line_inner(aide, history);
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

fn read_chat_line_inner(aide: &Aide, history: &[String]) -> anyhow::Result<Option<String>> {
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
    let mut current_theme = get_theme(&initial_theme_name, &custom_themes);

    let mut buf = String::new();
    let mut cursor_pos: usize = 0;
    let mut hist_idx: Option<usize> = None;
    let mut menu_sel: i32 = -1;

    loop {
        // Mode detection
        let is_theme_submode = buf.starts_with("/theme ");
        let show_menu = buf.starts_with('/');
        
        let (menu_items, filtered): (Vec<(String, String)>, Vec<usize>) = if is_theme_submode {
            let filter_text = &buf[7..];
            let items: Vec<(String, String)> = themes_list.iter().map(|t| (t.name.clone(), "theme".to_string())).collect();
            let filtered_indices: Vec<usize> = items.iter()
                .enumerate()
                .filter(|(_, (name, _))| name.starts_with(filter_text))
                .map(|(i, _)| i)
                .collect();
            (items, filtered_indices)
        } else if show_menu {
            let items: Vec<(String, String)> = CMDS.iter().map(|(c, d)| (c.to_string(), d.to_string())).collect();
            let filtered_indices: Vec<usize> = items.iter()
                .enumerate()
                .filter(|(_, (cmd, _))| cmd.starts_with(&buf))
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

        // Preview logic
        if is_theme_submode && menu_sel >= 0 {
            let theme_idx = filtered[menu_sel as usize];
            current_theme = themes_list[theme_idx].clone();
        } else {
            current_theme = get_theme(&initial_theme_name, &custom_themes);
        }

        let vis_cursor = buf[..cursor_pos].chars().count() as u16;
        let menu_rows = filtered.len() as u16;
        let prompt_label = " You ";
        let prompt_sep = " > ";
        let prompt_cols: u16 = (prompt_label.len() + prompt_sep.len()) as u16;

        // ── REDRAW ──
        crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        
        // Background for the whole line
        crossterm::execute!(stdout(), SetBackgroundColor(current_theme.user_bg_color()))?;
        crossterm::execute!(stdout(), Clear(ClearType::CurrentLine))?;
        
        // Print " You " (Bold color)
        crossterm::execute!(
            stdout(),
            SetForegroundColor(current_theme.bold_color()),
            SetAttribute(Attribute::Bold),
        )?;
        print!("{}", prompt_label);
        
        // Print " > " (Headers color)
        crossterm::execute!(
            stdout(),
            SetForegroundColor(current_theme.headers_color()),
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(current_theme.user_bg_color()),
        )?;
        print!("{}", prompt_sep);
        
        // Print buffer (User FG color)
        crossterm::execute!(stdout(), SetForegroundColor(current_theme.user_fg_color()))?;
        print!("{}", buf);
        
        // Ensure background spans full width
        crossterm::execute!(stdout(), Clear(ClearType::UntilNewLine))?;
        crossterm::execute!(stdout(), ResetColor)?;

        if show_menu && !filtered.is_empty() {
            for (menu_i, &idx) in filtered.iter().enumerate() {
                let (name, desc) = &menu_items[idx];
                if menu_sel == menu_i as i32 {
                    print!("\r\n  \x1b[7m{:<15}  {}\x1b[0m", name, desc);
                } else {
                    print!("\r\n  \x1b[2m{:<15}  {}\x1b[0m", name, desc);
                }
            }
            crossterm::execute!(stdout(), MoveUp(menu_rows), MoveToColumn(prompt_cols + vis_cursor))?;
        } else {
            crossterm::execute!(stdout(), MoveToColumn(prompt_cols + vis_cursor))?;
        }
        stdout().flush()?;

        let event = crossterm::event::read()?;
        if let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event {
            if kind != KeyEventKind::Press { continue; }
            match code {
                KeyCode::Enter => {
                    if menu_sel >= 0 && !filtered.is_empty() {
                        let selected = &menu_items[filtered[menu_sel as usize]].0;
                        if selected == "/theme" && !is_theme_submode {
                            buf = "/theme ".to_string();
                            cursor_pos = buf.len();
                            menu_sel = 0;
                            continue;
                        } else if is_theme_submode {
                            buf = format!("/theme {}", selected);
                        } else {
                            buf = selected.clone();
                        }
                    }
                    
                    if buf.trim() == "/" {
                        buf.clear(); cursor_pos = 0; menu_sel = -1; continue;
                    }

                    // Final render of the submitted line
                    let final_theme = if buf.starts_with("/theme ") {
                        get_theme(&buf[7..], &custom_themes)
                    } else {
                        get_theme(&initial_theme_name, &custom_themes)
                    };

                    crossterm::execute!(
                        stdout(), 
                        MoveToColumn(0), 
                        SetBackgroundColor(final_theme.user_bg_color()), 
                        Clear(ClearType::FromCursorDown)
                    )?;
                    crossterm::execute!(
                        stdout(),
                        SetForegroundColor(final_theme.bold_color()),
                        SetAttribute(Attribute::Bold),
                    )?;
                    print!("{}", prompt_label);
                    crossterm::execute!(
                        stdout(),
                        SetForegroundColor(final_theme.headers_color()),
                        SetAttribute(Attribute::Reset),
                        SetBackgroundColor(final_theme.user_bg_color()),
                    )?;
                    print!("{}", prompt_sep);
                    crossterm::execute!(stdout(), SetForegroundColor(final_theme.user_fg_color()))?;
                    print!("{}", buf);
                    crossterm::execute!(stdout(), Clear(ClearType::UntilNewLine), ResetColor)?;
                    print!("\r\n");
                    stdout().flush()?;
                    return Ok(Some(buf));
                }
                KeyCode::Tab => {
                    if menu_sel >= 0 && !filtered.is_empty() {
                        let selected = &menu_items[filtered[menu_sel as usize]].0;
                        if is_theme_submode {
                            buf = format!("/theme {}", selected);
                        } else if selected == "/theme" {
                            buf = "/theme ".to_string();
                        } else {
                            buf = selected.clone();
                        }
                        cursor_pos = buf.len();
                        menu_sel = 0;
                    }
                }
                KeyCode::Esc => {
                    if show_menu || menu_sel >= 0 {
                        buf.clear(); cursor_pos = 0; menu_sel = -1;
                    } else {
                        return Ok(Some(String::new()));
                    }
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    print!("\r\n"); stdout().flush()?; return Ok(None);
                }
                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    if buf.is_empty() { print!("\r\n"); stdout().flush()?; return Ok(None); }
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        menu_sel = if menu_sel <= 0 { filtered.len() as i32 - 1 } else { menu_sel - 1 };
                    } else if !history.is_empty() {
                        let new_idx = match hist_idx {
                            None => history.len() - 1,
                            Some(i) if i > 0 => i - 1,
                            Some(i) => i,
                        };
                        hist_idx = Some(new_idx); buf = history[new_idx].clone(); cursor_pos = buf.len();
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        menu_sel = if menu_sel >= filtered.len() as i32 - 1 { 0 } else { menu_sel + 1 };
                    } else {
                        match hist_idx {
                            None => {}
                            Some(i) if i + 1 >= history.len() => { hist_idx = None; buf.clear(); cursor_pos = 0; }
                            Some(i) => { hist_idx = Some(i + 1); buf = history[i + 1].clone(); cursor_pos = buf.len(); }
                        }
                    }
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char(cursor_pos, &buf);
                        buf.drain(prev..cursor_pos); cursor_pos = prev; menu_sel = -1; hist_idx = None;
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
                    buf.insert(cursor_pos, c); cursor_pos += c.len_utf8(); menu_sel = -1; hist_idx = None;
                }
                _ => {}
            }
        }
    }
}
