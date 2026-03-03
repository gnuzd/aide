use crate::memory::{MemoryStore, generate_session_id};
use crate::models::inference::InferenceEngine;
use crate::models::ModelRegistry;
use crate::system::SystemSpecs;
use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Parser)]
#[command(name = "aide")]
#[command(about = "A local-first, intelligent CLI assistant", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start an interactive chat session
    Chat,
    /// Run initial setup and hardware audit
    Setup,
    /// Get system information
    System,
    /// List available models
    Models,
    /// Clear saved data (conversations, profile, models, config)
    Clear,
    /// List, switch, or create color themes
    Theme,
}

pub async fn run_setup() -> anyhow::Result<()> {
    println!("{}", "=== Aide Setup Wizard ===".bold().cyan());

    let specs = SystemSpecs::audit();
    println!("\n{}", "1. Auditing Hardware...".bold());
    println!("- OS: {} {}", specs.os_name, specs.os_version);
    println!(
        "- CPU: {} ({} cores / {} threads)",
        specs.cpu_brand, specs.cpu_cores, specs.cpu_threads
    );
    println!(
        "- RAM: {} GB total ({} GB available)",
        specs.total_memory_gb, specs.available_memory_gb
    );

    let (compatible, warnings) = specs.check_compatibility();
    if !compatible {
        println!(
            "\n{}",
            "⚠️  Minimum hardware requirements not met!".red().bold()
        );
    }

    for warning in warnings {
        println!("  - {}", warning.yellow());
    }

    println!("\n{}", "2. Select Main Model...".bold());
    let registry = ModelRegistry::new();
    let compatible_models = registry.get_compatible_models(&specs);

    if compatible_models.is_empty() {
        println!(
            "{}",
            "No suitable local models found for your hardware.".red()
        );
    } else {
        let items: Vec<String> = compatible_models
            .iter()
            .map(|m| {
                format!(
                    "{} - {} ({} GB RAM min)",
                    m.name, m.description, m.min_ram_gb
                )
            })
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Pick a model to download as your main model")
            .items(&items)
            .default(0)
            .interact()
            .unwrap();

        let selected_model = compatible_models[selection];
        println!("\nYou selected: {}", selected_model.name.green().bold());

        // Download the model
        let model_path = registry.download_model(selected_model).await?;

        // Save to config
        let mut config = registry.load_config()?;
        config.active_model_path = Some(model_path);
        config.active_model_template = Some(selected_model.template_type.clone());
        registry.save_config(&config)?;

        println!("Model saved and activated!");
    }

    println!("\n{}", "Setup complete! Starting chat mode...".green());
    let _ = run_chat_loop();
    Ok(())
}

pub fn run_chat_loop() -> anyhow::Result<()> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;
    use std::io;

    let registry = ModelRegistry::new();
    let config = registry.load_config()?;

    // Extract theme config before config is partially moved by destructuring below
    let initial_theme_name = config.active_theme.clone().unwrap_or_else(|| "gruvbox".to_string());
    let initial_custom_themes = config.custom_themes.clone();

    let (model_path, template) = match (config.active_model_path, config.active_model_template) {
        (Some(path), Some(temp)) => (path, temp),
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

    println!("{}", "\n=== Aide Chat Mode ===".bold().cyan());
    println!("Type 'exit' or 'quit' to end the session.\n");

    let mut rl = DefaultEditor::new()?;
    let mut turn_number = 0u32;
    let stop = Arc::new(AtomicBool::new(false));

    let all_themes_init = crate::theme::all_themes(&initial_custom_themes);
    let mut current_theme = all_themes_init
        .into_iter()
        .find(|t| t.name == initial_theme_name)
        .unwrap_or_else(crate::theme::gruvbox);
    let mut md_skin = crate::ui::theme_to_skin(&current_theme);
    let mut hl = crate::ui::CodeHighlighter::with_syntax_theme(&current_theme.syntax_theme);

    // Append live config context so Aide can answer questions about current settings
    system_prompt.push_str(&format!(" Active theme: {}.", current_theme.name));

    loop {
        let readline = rl.readline(&format!("{} > ", "You".green().bold()));
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }

                // Slash commands — handled in-chat without going to the model
                if line.starts_with('/') {
                    let _ = rl.add_history_entry(line);
                    match line {
                        "/clear" => {
                            if let Err(e) = run_clear() {
                                println!("{}: {}", "Error".red().bold(), e);
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
                        "/models" => list_models(),
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
                                            md_skin = crate::ui::theme_to_skin(&t);
                                            hl = crate::ui::CodeHighlighter::with_syntax_theme(
                                                &t.syntax_theme,
                                            );
                                            current_theme = t;
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

                let _ = rl.add_history_entry(line);

                // Restyle the rustyline input line with a background colour
                let term_width = crossterm::terminal::size()
                    .map(|(w, _)| w as usize)
                    .unwrap_or(80);
                let pad = term_width.saturating_sub(line.chars().count() + 1);
                let (ubr, ubg, ubb) = crate::theme::parse_hex(&current_theme.user_bg);
                let (ufr, ufg, ufb) = crate::theme::parse_hex(&current_theme.user_fg);
                let _ = crossterm::execute!(
                    io::stdout(),
                    crossterm::cursor::MoveUp(1),
                    crossterm::cursor::MoveToColumn(0),
                    crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine),
                    crossterm::style::SetBackgroundColor(crossterm::style::Color::Rgb { r: ubr, g: ubg, b: ubb }),
                    crossterm::style::SetForegroundColor(crossterm::style::Color::Rgb { r: ufr, g: ufg, b: ufb }),
                    crossterm::style::Print(format!("{}\n", " ".repeat(term_width))),
                    crossterm::style::Print(format!(" {}{}\n", line, " ".repeat(pad))),
                    crossterm::style::Print(format!("{}\n", " ".repeat(term_width))),
                    crossterm::style::ResetColor,
                );
                println!();

                // Spinner + ESC/Ctrl-C watcher during generation
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .template("{spinner:.cyan} {msg}")
                        .unwrap(),
                );
                pb.set_message("Thinking...");

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

                let _ = crossterm::terminal::enable_raw_mode();
                pb.enable_steady_tick(std::time::Duration::from_millis(80));

                let result = engine.generate(line, 512, &system_prompt, &stop);
                let was_interrupted = stop.load(Ordering::Relaxed);
                stop.store(true, Ordering::Relaxed);
                let _ = esc_watcher.join();
                pb.finish_and_clear();
                let _ = crossterm::terminal::disable_raw_mode();

                match result {
                    Ok(response) => {
                        crate::ui::render_response(&response, &md_skin, &hl);
                        if was_interrupted {
                            println!("{}", "[stopped]".dimmed());
                        }
                        println!();
                        println!();

                        turn_number += 1;
                        if let Some(ref mem) = memory {
                            if let Err(e) = mem.save_turn(&session_id, turn_number, line, &response) {
                                eprintln!("Warning: Could not save turn: {}", e);
                            }
                            if let Err(e) = mem.extract_and_learn(line) {
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
            Err(ReadlineError::Interrupted) => {
                println!("\nSession ended.");
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}

pub fn run_clear() -> anyhow::Result<()> {
    println!("{}", "=== Aide: Clear Data ===".bold().cyan());

    let aide_dir = dirs::home_dir()
        .map(|h| h.join(".aide"))
        .unwrap_or_else(|| PathBuf::from(".aide"));

    if !aide_dir.exists() {
        println!("Nothing to clear — ~/.aide/ does not exist.");
        return Ok(());
    }

    let memory = MemoryStore::init_db(&aide_dir).ok();

    enum Action {
        Conversations,
        Profile,
        RememberedFacts,
        Model(PathBuf, String),
        Config,
        Everything,
    }

    let mut labels: Vec<String> = vec![];
    let mut actions: Vec<Action> = vec![];

    // Memory entries
    if let Some(ref mem) = memory {
        let (turns, sessions) = mem.conversation_stats().unwrap_or((0, 0));
        if turns > 0 {
            labels.push(format!(
                "Conversation history  ({} turns across {} sessions)",
                turns, sessions
            ));
            actions.push(Action::Conversations);
        }

        let profile_entries = mem.profile_entry_count();
        if profile_entries > 0 {
            labels.push(format!(
                "User profile  ({} learned entries — languages, skill level, topics, turn count)",
                profile_entries
            ));
            actions.push(Action::Profile);
        }

        let remembered = mem.remembered_facts_count();
        if remembered > 0 {
            labels.push(format!("Remembered facts only  ({} items)", remembered));
            actions.push(Action::RememberedFacts);
        }
    }

    // Downloaded models
    let models_dir = aide_dir.join("models");
    if models_dir.exists() {
        let mut model_entries: Vec<_> = std::fs::read_dir(&models_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().map(|x| x == "gguf").unwrap_or(false))
            .collect();
        model_entries.sort_by_key(|e| e.file_name());
        for entry in model_entries {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size_gb =
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) as f64 / 1_073_741_824.0;
            labels.push(format!("Model: {}  ({:.1} GB)", name, size_gb));
            actions.push(Action::Model(path, name));
        }
    }

    // Config
    let config_path = aide_dir.join("config.json");
    if config_path.exists() {
        labels.push(
            "Config  (active model selection — run `aide setup` to reconfigure)".to_string(),
        );
        actions.push(Action::Config);
    }

    // Nuclear option
    labels.push(format!("Everything  — delete all of {}/", aide_dir.display()));
    actions.push(Action::Everything);

    // Select — interact_opt returns None on ESC
    let selections = match MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select what to clear  (space to toggle, enter to confirm)")
        .items(&labels)
        .interact_opt()?
    {
        Some(s) if !s.is_empty() => s,
        _ => {
            println!("Cancelled.");
            return Ok(());
        }
    };

    // Summary before confirm
    println!("\n{}", "Will clear:".bold().yellow());
    for &i in &selections {
        let short = labels[i].split("  ").next().unwrap_or(&labels[i]);
        println!("  - {}", short);
    }
    println!();

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("This cannot be undone. Proceed?")
        .default(false)
        .interact_opt()?
        .unwrap_or(false);

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    println!();

    // "Everything" short-circuits all other selections
    if selections.iter().any(|&i| matches!(actions[i], Action::Everything)) {
        std::fs::remove_dir_all(&aide_dir)?;
        println!("{}", "Deleted ~/.aide/ — Aide has been completely reset.".green().bold());
        return Ok(());
    }

    for &i in &selections {
        match &actions[i] {
            Action::Conversations => {
                if let Some(ref mem) = memory {
                    mem.clear_conversations()?;
                    println!("Cleared conversation history.");
                }
            }
            Action::Profile => {
                if let Some(ref mem) = memory {
                    mem.clear_profile()?;
                    println!("Cleared user profile.");
                }
            }
            Action::RememberedFacts => {
                if let Some(ref mem) = memory {
                    mem.clear_remembered_facts()?;
                    println!("Cleared remembered facts.");
                }
            }
            Action::Model(path, name) => {
                std::fs::remove_file(path)?;
                println!("Deleted model: {}", name);
                // Reset config if this was the active model
                let registry = ModelRegistry::new();
                if let Ok(mut config) = registry.load_config() {
                    if config.active_model_path.as_ref() == Some(path) {
                        config.active_model_path = None;
                        config.active_model_template = None;
                        let _ = registry.save_config(&config);
                        println!("  (active model cleared — run `aide setup` to select a new one)");
                    }
                }
            }
            Action::Config => {
                std::fs::remove_file(&config_path)?;
                println!("Cleared config.");
            }
            Action::Everything => unreachable!(),
        }
    }

    println!("\n{}", "Done.".green().bold());
    Ok(())
}

pub fn run_theme() -> anyhow::Result<()> {
    println!("{}", "=== Aide: Themes ===".bold().cyan());

    let registry = ModelRegistry::new();
    let config = registry.load_config()?;
    let active = config
        .active_theme
        .clone()
        .unwrap_or_else(|| "gruvbox".to_string());
    let all = crate::theme::all_themes(&config.custom_themes);

    // ── List all themes ──────────────────────────────────────────────────────
    println!();
    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();
    for t in &all {
        let kind = if builtin_names.contains(&t.name) { "built-in" } else { "custom" };
        let marker = if t.name == active { " ← active".green().bold().to_string() } else { String::new() };
        println!("  {:<14}  [{}]{}", t.name.bold(), kind, marker);
    }
    println!();

    // ── Top-level menu ───────────────────────────────────────────────────────
    let menu = &["Switch theme", "Create new theme", "Cancel"];
    let choice = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(menu)
        .default(0)
        .interact_opt()?
    {
        Some(i) => i,
        None => return Ok(()),
    };

    match choice {
        0 => theme_switch(&registry, &config, &all, &active)?,
        1 => theme_create(&registry, &config)?,
        _ => {}
    }
    Ok(())
}

fn theme_switch(
    registry: &ModelRegistry,
    config: &crate::models::Config,
    all: &[crate::theme::Theme],
    active: &str,
) -> anyhow::Result<()> {
    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();
    let items: Vec<String> = all
        .iter()
        .map(|t| {
            let kind = if builtin_names.contains(&t.name) { "built-in" } else { "custom" };
            let marker = if t.name == active { "  ← active" } else { "" };
            format!("{:<14}  [{}]{}", t.name, kind, marker)
        })
        .collect();

    let default_idx = all.iter().position(|t| t.name == active).unwrap_or(0);
    let idx = match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select theme")
        .items(&items)
        .default(default_idx)
        .interact_opt()?
    {
        Some(i) => i,
        None => return Ok(()),
    };

    let selected = &all[idx];
    let mut new_config = config.clone();
    new_config.active_theme = Some(selected.name.clone());
    registry.save_config(&new_config)?;
    println!("\nSwitched to {} theme.", selected.name.green().bold());
    Ok(())
}

fn theme_create(
    registry: &ModelRegistry,
    config: &crate::models::Config,
) -> anyhow::Result<()> {
    println!("{}", "\nCreate New Theme".bold());
    println!("Enter hex colors in {} format. Press Enter to keep the default.\n", "#rrggbb".cyan());

    let validate_hex = |s: &String| -> Result<(), String> {
        if crate::theme::is_valid_hex(s) {
            Ok(())
        } else {
            Err(format!("Expected #rrggbb format, got '{}'", s))
        }
    };

    let builtin_names: Vec<_> = crate::theme::builtin_themes()
        .into_iter()
        .map(|t| t.name)
        .collect();

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Theme name")
        .validate_with(|s: &String| -> Result<(), String> {
            if s.trim().is_empty() {
                return Err("Name cannot be empty.".to_string());
            }
            if builtin_names.iter().any(|n| n == s.trim()) {
                return Err(format!("'{}' is a built-in theme and cannot be overridden.", s.trim()));
            }
            Ok(())
        })
        .interact_text()?;

    println!("\n{}", "── Text ──".dimmed());
    let fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Text color (fg)")
        .default("#ebdbb2".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Headings ──".dimmed());
    let h1: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("H1 color")
        .default("#fe8019".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let headers: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("H2–H6 color")
        .default("#fabd2f".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Emphasis ──".dimmed());
    let bold: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Bold color")
        .default("#fe8019".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let italic: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Italic color")
        .default("#b8bb26".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Code ──".dimmed());
    let code_fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Code text color")
        .default("#8ec07c".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let code_bg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Code background")
        .default("#3c3836".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let bullet: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Bullet color")
        .default("#fabd2f".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Chat bubble ──".dimmed());
    let user_bg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Your message background")
        .default("#076678".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;
    let user_fg: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Your message text color")
        .default("#ebdbb2".to_string())
        .validate_with(&validate_hex)
        .interact_text()?;

    println!("\n{}", "── Syntax highlighting ──".dimmed());
    let syntax_options = [
        "base16-ocean.dark",
        "base16-eighties.dark",
        "base16-mocha.dark",
        "Solarized (dark)",
        "InspiredGitHub",
    ];
    let syntax_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Code syntax theme")
        .items(&syntax_options)
        .default(0)
        .interact()?;

    let theme = crate::theme::Theme {
        name: name.trim().to_string(),
        fg,
        h1,
        headers,
        bold,
        italic,
        code_fg,
        code_bg,
        bullet,
        user_bg,
        user_fg,
        syntax_theme: syntax_options[syntax_idx].to_string(),
    };

    let mut new_config = config.clone();
    // Replace if a custom theme with the same name already exists
    new_config.custom_themes.retain(|t| t.name != theme.name);
    new_config.custom_themes.push(theme.clone());
    new_config.active_theme = Some(theme.name.clone());
    registry.save_config(&new_config)?;

    println!("\nTheme '{}' created and activated!", theme.name.green().bold());
    Ok(())
}

pub fn show_system_info() {
    let specs = SystemSpecs::audit();
    println!("{:#?}", specs);
}

pub fn list_models() {
    let registry = ModelRegistry::new();
    println!("{:<20} | {:<10} | {:<8}", "Name", "Type", "Min RAM");
    println!("{}", "-".repeat(45));
    for model in &registry.models {
        println!(
            "{:<20} | {:<10?} | {:<8} GB",
            model.name, model.model_type, model.min_ram_gb
        );
    }
}
