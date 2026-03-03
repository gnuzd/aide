use crate::cli::{list_models, read_chat_line, run_clear, run_theme, show_system_info};
use crate::memory::{MemoryStore, generate_session_id};
use crate::models::inference::InferenceEngine;
use crate::models::stable_diffusion::StableDiffusionEngine;
use crate::models::ModelRegistry;
use anyhow::Context;
use base64::Engine as _;
use colored::*;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};

enum StreamState {
    OutsideCode,
    AwaitCodeLang,
    InCode,
}

#[derive(Clone, Copy, PartialEq)]
enum PreviewTool {
    Kitty,
    Chafa,
    None,
}

#[derive(Clone)]
struct ImageArtifact {
    ext: String,
    bytes: Vec<u8>,
}

fn image_ext_from_lang(lang: &str) -> Option<&'static str> {
    let normalized = lang.trim().to_lowercase();
    let token = normalized.split_whitespace().next().unwrap_or("");

    match token {
        "image-prompt" => Some("sd-prompt"),
        "svg" | "image/svg+xml" => Some("svg"),
        "png" | "image/png" => Some("png"),
        "jpg" | "jpeg" | "image/jpeg" => Some("jpg"),
        "webp" | "image/webp" => Some("webp"),
        "gif" | "image/gif" => Some("gif"),
        "bmp" | "image/bmp" => Some("bmp"),
        "tif" | "tiff" | "image/tiff" => Some("tiff"),
        "ppm" | "image/x-portable-pixmap" => Some("ppm"),
        "pgm" | "image/x-portable-graymap" => Some("pgm"),
        "pbm" | "image/x-portable-bitmap" => Some("pbm"),
        _ => None,
    }
}

fn preferred_image_ext_from_user_input(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();

    let has = |needle: &str| tokens.iter().any(|t| *t == needle) || lower.contains(&format!(".{}", needle));

    if has("png") {
        Some("png")
    } else if has("jpeg") || has("jpg") {
        Some("jpg")
    } else if has("webp") {
        Some("webp")
    } else if has("gif") {
        Some("gif")
    } else if has("bmp") {
        Some("bmp")
    } else if has("tiff") || has("tif") {
        Some("tiff")
    } else if has("svg") {
        Some("svg")
    } else if has("ppm") {
        Some("ppm")
    } else if has("pgm") {
        Some("pgm")
    } else if has("pbm") {
        Some("pbm")
    } else {
        None
    }
}

fn detect_preview_tool() -> PreviewTool {
    let is_kitty = std::env::var("TERM")
        .map(|t| t.contains("kitty"))
        .unwrap_or(false);

    if is_kitty
        && Command::new("kitten")
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    {
        return PreviewTool::Kitty;
    }

    if Command::new("chafa")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return PreviewTool::Chafa;
    }

    PreviewTool::None
}

fn print_text_stream(s: &str, skin: &termimad::MadSkin) {
    if s.is_empty() {
        return;
    }
    // Using print_text on small chunks causes vertical breaking. 
    // We print raw for the stream to keep it fluid.
    print!("{}", s.replace('\n', "\r\n"));
}

fn highlight_code_block(
    code: &str,
    lang: &str,
    syntax_theme: &str,
    ss: &SyntaxSet,
    ts: &ThemeSet,
) -> String {
    let syntax = if lang.is_empty() {
        ss.find_syntax_plain_text()
    } else {
        ss.find_syntax_by_token(lang)
            .unwrap_or_else(|| ss.find_syntax_plain_text())
    };

    let theme = ts
        .themes
        .get(syntax_theme)
        .or_else(|| ts.themes.get("base16-ocean.dark"))
        .or_else(|| ts.themes.values().next())
        .unwrap();

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut out = String::new();
    for line in LinesWithEndings::from(code) {
        if let Ok(ranges) = highlighter.highlight_line(line, ss) {
            out.push_str(&as_24_bit_terminal_escaped(&ranges, false));
        }
    }
    out.push_str("\x1b[0m");
    out
}

fn parse_fenced_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut pos = 0;

    while let Some(start_rel) = text[pos..].find("```") {
        let start = pos + start_rel;
        let lang_line_start = start + 3;
        let Some(nl_rel) = text[lang_line_start..].find('\n') else {
            break;
        };
        let lang_line_end = lang_line_start + nl_rel;
        let lang = text[lang_line_start..lang_line_end].trim().to_string();
        let body_start = lang_line_end + 1;
        let Some(end_rel) = text[body_start..].find("```") else {
            break;
        };
        let body_end = body_start + end_rel;
        blocks.push((lang, text[body_start..body_end].to_string()));
        pos = body_end + 3;
    }

    blocks
}

fn decode_image_block(ext: &str, body: &str) -> anyhow::Result<Vec<u8>> {
    if ext == "sd-prompt" || ext == "svg" {
        return Ok(body.as_bytes().to_vec());
    }

    let trimmed = body.trim();
    let payload = if let Some(idx) = trimmed.find("base64,") {
        &trimmed[idx + "base64,".len()..]
    } else {
        trimmed
    };
    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned: String = compact
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
        .collect();

    base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&cleaned))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&cleaned))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&cleaned))
        .context("invalid base64 image payload")
}

fn collect_image_artifacts(response: &str) -> (Vec<ImageArtifact>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (lang, body) in parse_fenced_blocks(response) {
        let Some(ext) = image_ext_from_lang(&lang) else {
            continue;
        };
        match decode_image_block(ext, &body) {
            Ok(bytes) => out.push(ImageArtifact {
                ext: ext.to_string(),
                bytes,
            }),
            Err(e) => errors.push(format!("{} artifact decode failed: {}", ext, e)),
        }
    }
    (out, errors)
}

fn image_artifacts_from_blocks(blocks: &[(String, String)]) -> (Vec<ImageArtifact>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (ext, body) in blocks {
        match decode_image_block(ext, body) {
            Ok(bytes) => out.push(ImageArtifact {
                ext: ext.clone(),
                bytes,
            }),
            Err(e) => errors.push(format!("{} artifact decode failed: {}", ext, e)),
        }
    }
    (out, errors)
}

fn preview_image_in_terminal(path: &Path, tool: PreviewTool) -> bool {
    match tool {
        PreviewTool::Kitty => Command::new("kitten")
            .arg("icat")
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false),
        PreviewTool::Chafa => Command::new("chafa")
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false),
        PreviewTool::None => false,
    }
}

fn maybe_handle_image_artifacts(
    artifacts: &[ImageArtifact],
    sd_engine: Option<&StableDiffusionEngine>,
) -> anyhow::Result<()> {
    if artifacts.is_empty() {
        return Ok(());
    }

    let preview_tool = detect_preview_tool();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for (idx, artifact) in artifacts.iter().enumerate() {
        let mut final_ext = artifact.ext.clone();
        let mut final_bytes = artifact.bytes.clone();

        if artifact.ext == "sd-prompt" {
            let prompt = String::from_utf8_lossy(&artifact.bytes);
            if let Some(sd) = sd_engine {
                println!(
                    "{} {}",
                    "Generating image with Stable Diffusion:".cyan(),
                    prompt.bold()
                );
                let tmp_gen_path = std::env::temp_dir().join(format!("sd-gen-{}.png", ts));
                if let Err(e) = sd.generate(&prompt, &tmp_gen_path) {
                    println!("{}: {}", "Generation failed".red(), e);
                    continue;
                }
                final_bytes = fs::read(&tmp_gen_path)?;
                final_ext = "png".to_string();
                let _ = fs::remove_file(tmp_gen_path);
            } else {
                println!(
                    "{}",
                    "No Stable Diffusion model configured. Cannot generate image.".red()
                );
                continue;
            }
        }

        let suggested_name = format!("aide-image-{}-{}.{}", ts, idx + 1, final_ext);
        println!("{} {}", "Image artifact:".cyan(), suggested_name.bold());

        let tmp_path = if preview_tool != PreviewTool::None {
            let p = std::env::temp_dir().join(format!("aide-preview-{}", suggested_name));
            fs::write(&p, &final_bytes)?;
            let _ = preview_image_in_terminal(&p, preview_tool);
            Some(p)
        } else {
            None
        };

        let save = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Save this image?")
            .default(true)
            .interact_opt()?
            .unwrap_or(false);

        if save {
            let mut output_name: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Output file")
                .default(suggested_name.clone())
                .interact_text()?;

            output_name = output_name.trim().to_string();
            if output_name.is_empty() {
                output_name = suggested_name.clone();
            }

            let mut output_path = {
                let p = PathBuf::from(&output_name);
                if p.is_absolute() { p } else { std::env::current_dir()?.join(p) }
            };
            if output_path.extension().is_none() {
                output_path.set_extension(&final_ext);
            }
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&output_path, &final_bytes)?;
            println!("{} {}", "Saved:".green().bold(), output_path.display());
        } else {
            println!("{}", "Discarded image artifact.".dimmed());
        }

        if let Some(p) = tmp_path {
            let _ = fs::remove_file(p);
        }
    }

    Ok(())
}

pub fn run_chat_loop() -> anyhow::Result<()> {
    use std::io::{self, Write};

    let registry = ModelRegistry::new();
    let config = registry.load_config()?;

    // Extract theme config before config is partially moved by destructuring below
    let initial_theme_name = config.active_theme.clone().unwrap_or_else(|| "gruvbox".to_string());
    let initial_custom_themes = config.custom_themes.clone();

    let (model_path, template, design_model_path) = match (config.active_model_path, config.active_model_template, config.active_design_model_path) {
        (Some(path), Some(temp), d_path) => (path, temp, d_path),
        _ => {
            println!(
                "{}",
                "No active model found. Please run `aide setup` first.".red()
            );
            return Ok(());
        }
    };

    // Initialize memory — best-effort, never crash if DB fails
    let aide_dir = dirs::home_dir()
        .map(|h| h.join(".aide"))
        .unwrap_or_else(|| PathBuf::from(".aide"));

    let memory: Option<MemoryStore> = match MemoryStore::init_db(&aide_dir) {
        Ok(mem) => Some(mem),
        Err(e) => {
            eprintln!("Warning: Could not initialize memory: {}", e);
            None
        }
    };

    let session_id = generate_session_id();

    // Build personalized system prompt from saved profile
    let mut system_prompt = if let Some(ref mem) = memory {
        match mem.get_profile_summary() {
            Ok(summary) => summary,
            Err(e) => {
                eprintln!("Warning: Could not load profile: {}", e);
                "You are Aide, a helpful assistant.".to_string()
            }
        }
    } else {
        "You are Aide, a helpful assistant.".to_string()
    };

    println!("{}", "Loading model...".dimmed());
    let engine = InferenceEngine::new(&model_path, template)?;
    let sd_engine = design_model_path.as_ref().map(|p| StableDiffusionEngine::new(p));

    println!("{}", "\n=== Aide Chat Mode ===".bold().cyan());
    println!("Type 'exit' or 'quit' to end the session.\n");

    let mut chat_history: Vec<String> = Vec::new();
    let mut turn_number = 0u32;
    let stop = Arc::new(AtomicBool::new(false));

    let all_themes_init = crate::theme::all_themes(&initial_custom_themes);
    let mut current_theme = all_themes_init
        .into_iter()
        .find(|t| t.name == initial_theme_name)
        .unwrap_or_else(crate::theme::gruvbox);
    let mut syntax_theme_name = current_theme.syntax_theme.clone();
    let mut skin = crate::ui::theme_to_skin(&current_theme);
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let preview_tool = detect_preview_tool();
    // Append live config context so Aide can answer questions about current settings
    system_prompt.push_str(&format!(" Active theme: {}.", current_theme.name));

    loop {
        let input = match read_chat_line(&chat_history)? {
            None => {
                println!("\nSession ended.");
                break;
            }
            Some(s) => s,
        };
        let line = input.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }

        // Slash commands — handled in-chat without going to the model
        if line.starts_with('/') {
            match line.as_str() {
                "/clear" => {
                    match run_clear() {
                        Ok(true) => {
                            println!("\n{}", "Aide needs setup. Exiting chat...".cyan().bold());
                            break;
                        }
                        Ok(false) => {}
                        Err(e) => println!("{}: {}", "Error".red().bold(), e),
                    }
                }
                "/memory" => {
                    if let Some(ref mem) = memory {
                        match mem.get_profile_summary() {
                            Ok(s) => println!("{}", s),
                            Err(e) => println!("{}: {}", "Error".red().bold(), e),
                        }
                    } else {
                        println!("Memory not available.");
                    }
                }
                "/models" => {
                    if let Err(e) = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(list_models())
                    }) {
                        println!("{}: {}", "Error".red().bold(), e);
                    }
                }
                "/system" => show_system_info(),
                "/theme" => {
                    match run_theme() {
                        Ok(()) => {
                            // Reload config and apply the newly selected theme live
                            let r = ModelRegistry::new();
                            if let Ok(cfg) = r.load_config() {
                                let name = cfg.active_theme.clone()
                                    .unwrap_or_else(|| "gruvbox".to_string());
                                let all = crate::theme::all_themes(&cfg.custom_themes);
                                if let Some(t) = all.into_iter().find(|t| t.name == name) {
                                    syntax_theme_name = t.syntax_theme.clone();
                                    current_theme = t;
                                    skin = crate::ui::theme_to_skin(&current_theme);
                                }
                            }
                        }
                        Err(e) => println!("{}: {}", "Error".red().bold(), e),
                    }
                }
                "/help" => {
                    println!("{}", "In-chat commands:".bold());
                    println!("  /clear    clear data (conversations, profile, models, config)");
                    println!("  /memory   show what Aide currently knows about you");
                    println!("  /models   list available models");
                    println!("  /system   show system information");
                    println!("  /theme    list, switch, or create color themes");
                    println!("  /help     show this help");
                    println!("  exit      end the session");
                }
                _ => println!(
                    "Unknown command: {}  (type /help for available commands)",
                    line
                ),
            }
            println!();
            continue;
        }

        chat_history.push(line.clone());

        // Restyle the input line as a compact coloured chat bubble
        let (ubr, ubg, ubb) = crate::theme::parse_hex(&current_theme.user_bg);
        let (ufr, ufg, ufb) = crate::theme::parse_hex(&current_theme.user_fg);
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::cursor::MoveUp(1),
            crossterm::cursor::MoveToColumn(0),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine),
            crossterm::style::SetBackgroundColor(crossterm::style::Color::Rgb { r: ubr, g: ubg, b: ubb }),
            crossterm::style::SetForegroundColor(crossterm::style::Color::Rgb { r: ufr, g: ufg, b: ufb }),
            crossterm::style::Print(format!(" {} ", line)),
            crossterm::style::ResetColor,
            crossterm::style::Print("\n"),
        );
        println!();

        stop.store(false, Ordering::Relaxed);
        let stop_watcher = stop.clone();
        let esc_watcher = std::thread::spawn(move || {
            use crossterm::event::{Event, KeyCode, KeyModifiers};
            while !stop_watcher.load(Ordering::Relaxed) {
                if crossterm::event::poll(std::time::Duration::from_millis(50))
                    .unwrap_or(false)
                {
                    if let Ok(Event::Key(key)) = crossterm::event::read() {
                        let is_esc = key.code == KeyCode::Esc;
                        let is_ctrl_c = key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL);
                        if is_esc || is_ctrl_c {
                            stop_watcher.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            }
        });

        let turn_system_prompt = system_prompt.clone();

        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        pb.set_message("Thinking...");
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let _ = crossterm::terminal::enable_raw_mode();
        let mut started = false;
        let mut state = StreamState::OutsideCode;
        let mut pending = String::new();
        let mut code_lang = String::new();
        let mut code_buf = String::new();
        let mut captured_image_blocks: Vec<(String, String)> = Vec::new();
        let mut image_counter = 0usize;
        let _ = io::stdout().flush();
        let pb_in_cb = pb.clone();
        let result = engine.generate_stream(&line, 512, &turn_system_prompt, &stop, |piece| {
            if !started {
                pb_in_cb.finish_and_clear();
                started = true;
            }
            pending.push_str(piece);

            loop {
                match state {
                    StreamState::OutsideCode => {
                        if let Some(idx) = pending.find("```") {
                            print_text_stream(&pending[..idx], &skin);
                            let _ = io::stdout().flush();
                            pending.drain(..idx + 3);
                            state = StreamState::AwaitCodeLang;
                            continue;
                        }

                        // Keep a tiny suffix to catch split fences across chunks.
                        // Must only slice at char boundaries.
                        if pending.len() > 3 {
                            let mut flush_to = pending.len() - 3;
                            while flush_to > 0 && !pending.is_char_boundary(flush_to) {
                                flush_to -= 1;
                            }
                            if flush_to > 0 {
                                let to_print = pending[..flush_to].to_string();
                                pending.drain(..flush_to);
                                print_text_stream(&to_print, &skin);
                                let _ = io::stdout().flush();
                            }
                        }
                        break;
                    }
                    StreamState::AwaitCodeLang => {
                        if let Some(nl) = pending.find('\n') {
                            code_lang = pending[..nl].trim().to_string();
                            pending.drain(..nl + 1);
                            state = StreamState::InCode;
                            continue;
                        }
                        break;
                    }
                    StreamState::InCode => {
                        if let Some(idx) = pending.find("```") {
                            code_buf.push_str(&pending[..idx]);
                            pending.drain(..idx + 3);
                            if let Some(ext) = image_ext_from_lang(&code_lang) {
                                image_counter += 1;
                                captured_image_blocks.push((ext.to_string(), code_buf.clone()));
                                if preview_tool == PreviewTool::None {
                                    print_text_stream(&format!(
                                        "\n[image artifact: image-{}.{}]\n",
                                        image_counter, ext
                                    ), &skin);
                                } else {
                                    print_text_stream(&format!(
                                        "\n[image artifact detected: image-{}.{}]\n",
                                        image_counter, ext
                                    ), &skin);
                                }
                            } else {
                                let highlighted = highlight_code_block(
                                    &code_buf,
                                    &code_lang,
                                    &syntax_theme_name,
                                    &ss,
                                    &ts,
                                );
                                print_text_stream(&highlighted, &skin);
                            }
                            let _ = io::stdout().flush();
                            code_buf.clear();
                            code_lang.clear();
                            state = StreamState::OutsideCode;
                            continue;
                        }

                        if pending.len() > 3 {
                            let mut keep_at = pending.len() - 3;
                            while keep_at > 0 && !pending.is_char_boundary(keep_at) {
                                keep_at -= 1;
                            }
                            if keep_at > 0 {
                                let to_add = pending[..keep_at].to_string();
                                code_buf.push_str(&to_add);
                                pending.drain(..keep_at);
                            }
                        }
                        break;
                    }
                }
            }
        });

        // Flush any remaining buffered output.
        match state {
            StreamState::OutsideCode | StreamState::AwaitCodeLang => {
                if !pending.is_empty() {
                    print_text_stream(&pending, &skin);
                }
            }
            StreamState::InCode => {
                code_buf.push_str(&pending);
                let highlighted = highlight_code_block(
                    &code_buf,
                    &code_lang,
                    &syntax_theme_name,
                    &ss,
                    &ts,
                );
                print_text_stream(&highlighted, &skin);
            }
        }
        let _ = io::stdout().flush();

        let was_interrupted = stop.load(Ordering::Relaxed);
        stop.store(true, Ordering::Relaxed);
        let _ = esc_watcher.join();
        pb.finish_and_clear();
        let _ = crossterm::terminal::disable_raw_mode();
        println!();

        match result {
            Ok(response) => {
                if was_interrupted {
                    println!("{}", "[stopped]".dimmed());
                } else {
                    // Re-render the full response with proper markdown skin and syntax highlighting
                    // This cleans up any split line artifacts from streaming
                    let hl = crate::ui::CodeHighlighter::with_syntax_theme(&syntax_theme_name);
                    let _ = crossterm::execute!(io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine));
                    println!("\r"); // Move to start of line
                    crate::ui::render_response(&response, &skin, &hl);
                }
                println!();
                println!();
                if !was_interrupted {
                    let (mut artifacts, mut decode_errors) =
                        image_artifacts_from_blocks(&captured_image_blocks);
                    if artifacts.is_empty() {
                        let (fallback_artifacts, fallback_errors) = collect_image_artifacts(&response);
                        artifacts = fallback_artifacts;
                        decode_errors.extend(fallback_errors);
                    }
                    if let Err(e) = maybe_handle_image_artifacts(&artifacts, sd_engine.as_ref()) {
                        eprintln!("Warning: Could not process image artifact: {}", e);
                    }
                    if artifacts.is_empty() && !decode_errors.is_empty() {
                        eprintln!("Warning: Image artifact was detected but could not be decoded.");
                        for err in decode_errors {
                            eprintln!("  - {}", err);
                        }
                    }
                }

                turn_number += 1;
                if let Some(ref mem) = memory {
                    if let Err(e) = mem.save_turn(&session_id, turn_number, &line, &response) {
                        eprintln!("Warning: Could not save turn: {}", e);
                    }
                    if let Err(e) = mem.extract_and_learn(&line) {
                        eprintln!("Warning: Could not update profile: {}", e);
                    }
                    // Refresh system prompt so remembered facts take effect next turn
                    match mem.get_profile_summary() {
                        Ok(mut updated) => {
                            updated.push_str(&format!(" Active theme: {}.", current_theme.name));
                            system_prompt = updated;
                        }
                        Err(e) => eprintln!("Warning: Could not refresh profile: {}", e),
                    }
                }
            }
            Err(e) => {
                println!("{}: {}", "Error".red().bold(), e);
                println!();
            }
        }
    }
    Ok(())
}
