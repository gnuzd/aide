use colored::*;


const CMDS: &[(&str, &str)] = &[
    ("/clear",  "clear data (conversations, profile, models, config)"),
    ("/help",   "show this help"),
    ("/memory", "show what Aide currently knows about you"),
    ("/models", "list available models"),
    ("/system", "show system information"),
    ("/theme",  "list, switch, or create color themes"),
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

/// Read one line of chat input with a live slash-command dropdown.
/// Returns `Ok(None)` on Ctrl-C / Ctrl-D (caller should break the chat loop).
pub fn read_chat_line(history: &[String]) -> anyhow::Result<Option<String>> {
    crossterm::terminal::enable_raw_mode()?;
    let result = read_chat_line_inner(history);
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

fn read_chat_line_inner(history: &[String]) -> anyhow::Result<Option<String>> {
    use crossterm::cursor::{MoveToColumn, MoveUp};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use crossterm::terminal::{Clear, ClearType};
    use std::io::{Write, stdout};

    // Visual prompt: "You > " with colour.  PROMPT_COLS is the visual width.
    let prompt_display = format!("{} > ", "You".green().bold());
    const PROMPT_COLS: u16 = 6; // "You > " = 6 visible chars

    let mut buf = String::new();
    let mut cursor_pos: usize = 0; // byte offset in buf
    let mut hist_idx: Option<usize> = None;
    let mut menu_sel: i32 = -1; // -1 = nothing highlighted

    print!("{}", prompt_display);
    stdout().flush()?;

    loop {
        // Build filtered command list based on current buffer
        let show_menu = buf.starts_with('/');
        let filtered: Vec<usize> = if show_menu {
            CMDS.iter()
                .enumerate()
                .filter(|(_, (cmd, _))| cmd.starts_with(buf.as_str()))
                .map(|(i, _)| i)
                .collect()
        } else {
            vec![]
        };

        // Clamp selection to valid range
        if filtered.is_empty() {
            menu_sel = -1;
        } else {
            menu_sel = menu_sel.min(filtered.len() as i32 - 1);
        }

        let vis_cursor = buf[..cursor_pos].chars().count() as u16;
        let menu_rows = filtered.len() as u16;

        // ── Redraw: clear current line + any menu below, then repaint ──
        crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        print!("{}{}", prompt_display, buf);

        if show_menu && !filtered.is_empty() {
            for (menu_i, &cmd_i) in filtered.iter().enumerate() {
                let (cmd, desc) = CMDS[cmd_i];
                if menu_sel == menu_i as i32 {
                    print!("\r\n  \x1b[7m{:<10}  {}\x1b[0m", cmd, desc);
                } else {
                    print!("\r\n  \x1b[2m{:<10}  {}\x1b[0m", cmd, desc);
                }
            }
            crossterm::execute!(
                stdout(),
                MoveUp(menu_rows),
                MoveToColumn(PROMPT_COLS + vis_cursor)
            )?;
        } else {
            crossterm::execute!(stdout(), MoveToColumn(PROMPT_COLS + vis_cursor))?;
        }
        stdout().flush()?;

        // ── Handle key event ──
        let event = crossterm::event::read()?;
        if let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event {
            if kind != KeyEventKind::Press {
                continue;
            }
            match code {
                KeyCode::Enter => {
                    let result_str = if menu_sel >= 0 && !filtered.is_empty() {
                        CMDS[filtered[menu_sel as usize]].0.to_string()
                    } else if buf.trim() == "/" {
                        // bare "/" with no selection: clear and stay
                        buf.clear();
                        cursor_pos = 0;
                        menu_sel = -1;
                        continue;
                    } else {
                        buf.clone()
                    };
                    crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
                    print!("{}{}\r\n", prompt_display, result_str);
                    stdout().flush()?;
                    return Ok(Some(result_str));
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
                    print!("\r\n");
                    stdout().flush()?;
                    return Ok(None);
                }
                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    if buf.is_empty() {
                        crossterm::execute!(stdout(), MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
                        print!("\r\n");
                        stdout().flush()?;
                        return Ok(None);
                    }
                }
                KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                    if cursor_pos > 0 {
                        let new_pos = buf[..cursor_pos].trim_end().rfind(' ')
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        buf.drain(new_pos..cursor_pos);
                        cursor_pos = new_pos;
                        menu_sel = -1;
                    }
                }
                KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                    cursor_pos = 0;
                }
                KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                    cursor_pos = buf.len();
                }
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.drain(..cursor_pos);
                    cursor_pos = 0;
                    menu_sel = -1;
                }
                KeyCode::Home => cursor_pos = 0,
                KeyCode::End  => cursor_pos = buf.len(),
                KeyCode::Left  => cursor_pos = prev_char(cursor_pos, &buf),
                KeyCode::Right => cursor_pos = next_char(cursor_pos, &buf),
                KeyCode::Up => {
                    if show_menu && !filtered.is_empty() {
                        menu_sel = if menu_sel <= 0 {
                            filtered.len() as i32 - 1
                        } else {
                            menu_sel - 1
                        };
                    } else if !history.is_empty() {
                        let new_idx = match hist_idx {
                            None => history.len() - 1,
                            Some(i) if i > 0 => i - 1,
                            Some(i) => i,
                        };
                        hist_idx = Some(new_idx);
                        buf = history[new_idx].clone();
                        cursor_pos = buf.len();
                        menu_sel = -1;
                    }
                }
                KeyCode::Down => {
                    if show_menu && !filtered.is_empty() {
                        menu_sel = if menu_sel >= filtered.len() as i32 - 1 {
                            0
                        } else {
                            menu_sel + 1
                        };
                    } else {
                        match hist_idx {
                            None => {}
                            Some(i) if i + 1 >= history.len() => {
                                hist_idx = None;
                                buf.clear();
                                cursor_pos = 0;
                            }
                            Some(i) => {
                                hist_idx = Some(i + 1);
                                buf = history[i + 1].clone();
                                cursor_pos = buf.len();
                            }
                        }
                        menu_sel = -1;
                    }
                }
                KeyCode::Tab => {
                    if !filtered.is_empty() {
                        let sel = if menu_sel >= 0 { menu_sel as usize } else { 0 };
                        buf = CMDS[filtered[sel]].0.to_string();
                        cursor_pos = buf.len();
                        menu_sel = sel as i32;
                    }
                }
                KeyCode::Esc => {
                    if show_menu {
                        buf.clear();
                        cursor_pos = 0;
                        menu_sel = -1;
                    }
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char(cursor_pos, &buf);
                        buf.drain(prev..cursor_pos);
                        cursor_pos = prev;
                        menu_sel = -1;
                        hist_idx = None;
                    }
                }
                KeyCode::Delete => {
                    if cursor_pos < buf.len() {
                        let next = next_char(cursor_pos, &buf);
                        buf.drain(cursor_pos..next);
                        menu_sel = -1;
                    }
                }
                KeyCode::Char(c) => {
                    buf.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                    menu_sel = -1;
                    hist_idx = None;
                }
                _ => {}
            }
        }
    }
}
