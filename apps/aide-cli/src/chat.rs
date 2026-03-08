use crate::input::read_chat_line;
use crate::theme::{Theme, all_themes, get_theme};
use aide_core::Aide;
use aide_core::models::inference::InferenceEngine;
use aide_core::system::SystemSpecs;
use colored::*;
use crossterm::style::{
    Attribute, SetAttribute, SetForegroundColor,
};
use std::io::{Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, Clear, ClearType};

pub async fn run_chat_loop(
    aide: &mut Aide,
    engine: InferenceEngine,
    session_id: &str,
) -> anyhow::Result<()> {
    println!("{}", "\n=== Aide Chat Mode ===".bold().cyan());
    println!("Type '/' for commands or 'exit' to exit.\n");

    let mut chat_history: Vec<String> = Vec::new();
    if let Ok(recent) = aide.memory.load_recent_history(10) {
        for (u, a) in recent {
            chat_history.push(format!("User: {}", u));
            chat_history.push(format!("Assistant: {}", a));
        }
    }

    let stop = AtomicBool::new(false);
    let (stats_turns, _) = aide.memory.conversation_stats().unwrap_or((0, 0));
    let mut turn_number = stats_turns as u32;

    // Welcome Message
    if let Ok(welcome) = aide.generate_welcome_message(&engine).await {
        println!();
        let current_theme = get_theme(aide.config.active_theme.as_deref().unwrap_or("gruvbox"), &[]);
        crossterm::execute!(stdout(), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold))?;
        print!("Aide: ");
        crossterm::execute!(stdout(), SetAttribute(Attribute::Reset))?;
        print!("\r\n");
        current_theme.to_mad_skin().print_text(&welcome);
    }

    loop {
        println!();
        let input = match read_chat_line(aide)? {
            None => { println!("\nSession ended."); break; }
            Some(s) => s,
        };
        let line = input.trim().to_string();
        if line.is_empty() { continue; }
        if line == "exit" { 
            println!("\nSession ended.");
            break; 
        }

        let mut turn_system_prompt = aide.memory.get_profile_summary().unwrap_or_else(|_| "You are Aide, a helpful assistant.".to_string());
        if let Ok(semantic_facts) = aide.memory.search_semantic(&line, 3) {
            if !semantic_facts.is_empty() {
                turn_system_prompt.push_str("\nRelevant facts: ");
                turn_system_prompt.push_str(&semantic_facts.join("; "));
            }
        }

        let mut custom_themes = Vec::new();
        for v in &aide.config.custom_themes {
            if let Ok(t) = serde_json::from_value::<Theme>(v.clone()) { custom_themes.push(t); }
        }
        let current_theme = get_theme(aide.config.active_theme.as_deref().unwrap_or("gruvbox"), &custom_themes);

        if line.starts_with('/') {
            if handle_slash_command(&line, aide, &custom_themes)? {
                println!("\nSession ended.");
                break;
            }
            continue;
        }

        print!("\n");
        crossterm::execute!(stdout(), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold))?;
        print!("Aide: ");
        crossterm::execute!(stdout(), SetAttribute(Attribute::Reset), SetForegroundColor(current_theme.fg_color()))?;
        print!("{}", "Thinking...".dimmed());
        stdout().flush()?;

        let mut response_full = String::new();
        let mut first_token = true;

        let _ = enable_raw_mode();
        stop.store(false, Ordering::Relaxed);
        let res = engine.ask_stream(&line, &chat_history, 1024, &turn_system_prompt, &stop, |token| {
            if first_token {
                print!("\r");
                crossterm::execute!(stdout(), Clear(ClearType::CurrentLine), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold)).unwrap();
                print!("Aide: ");
                crossterm::execute!(stdout(), SetAttribute(Attribute::Reset), SetForegroundColor(current_theme.fg_color())).unwrap();
                first_token = false;
            }
            print!("{}", token.replace('\n', "\r\n"));
            stdout().flush().unwrap();
            response_full.push_str(token);

            while event::poll(Duration::from_millis(0)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press && (key.code == KeyCode::Esc || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))) {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
            }
        });
        let _ = disable_raw_mode();
        res?;

        if !stop.load(Ordering::Relaxed) {
            print!("\r");
            crossterm::execute!(stdout(), Clear(ClearType::CurrentLine), SetForegroundColor(current_theme.h1_color()), SetAttribute(Attribute::Bold))?;
            print!("Aide: ");
            crossterm::execute!(stdout(), SetAttribute(Attribute::Reset))?;
            print!("\r\n");
            current_theme.to_mad_skin().print_text(&response_full);
        }

        print!("\n");
        stdout().flush()?;

        turn_number += 1;
        let _ = aide.memory.save_turn(session_id, turn_number, &line, &response_full);
        chat_history.push(format!("User: {}", line));
        chat_history.push(format!("Assistant: {}", response_full));
    }
    Ok(())
}

fn handle_slash_command(line: &str, aide: &mut Aide, custom_themes: &[Theme]) -> anyhow::Result<bool> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let cmd = parts[0];
    match cmd {
        "/exit" => return Ok(true),
        "/help" => {
            println!("\n{}", "Available Commands:".bold());
            println!("  /bug        - Report an issue with logs");
            println!("  /clear      - Clear data (subcommands: all, chat, profile, models, config)");
            println!("  /exit       - Exit the application");
            println!("  /help       - Show this help");
            println!("  /memory     - Show what Aide knows about you");
            println!("  /models     - List available models");
            println!("  /system     - Show system information");
            println!("  /theme      - List or switch color themes");
        }
        "/bug" => {
            println!("\nTo report a bug, please visit: https://github.com/chrisnguyen/aide/issues");
            println!("You can include your logs located in: {:?}", aide.registry.base_path.join("aide.log"));
        }
        "/clear" => {
            if parts.len() < 2 {
                println!("\nUsage: /clear <all|chat|profile|models|config>");
            } else {
                match parts[1] {
                    "all" => { aide.reset()?; println!("All data cleared."); return Ok(true); }
                    "chat" => { aide.clear_conversations()?; println!("Chat history cleared."); }
                    "profile" => { aide.clear_profile()?; println!("User profile cleared."); }
                    "models" => { aide.clear_models()?; println!("Models cleared."); }
                    "config" => { aide.clear_config()?; println!("Config cleared."); }
                    _ => println!("Unknown sub-command for /clear."),
                }
            }
        }
        "/memory" => {
            match aide.memory.get_profile_summary() {
                Ok(summary) => {
                    println!("\n{}", "Aide's Memory of You:".bold());
                    println!("{}", summary);
                }
                Err(e) => println!("Error loading memory: {}", e),
            }
        }
        "/models" => {
            println!("\n{}", "Available Models:".bold());
            let models_dir = aide.registry.base_path.join("models");
            for model in &aide.registry.models {
                let model_path = models_dir.join(&model.filename);
                let is_downloaded = model_path.exists();
                let is_active = Some(model_path) == aide.config.active_model_path;

                let mut status = Vec::new();
                if is_active { status.push("Active".bold().green().to_string()); }
                if is_downloaded { status.push("Downloaded".dimmed().to_string()); }

                let status_str = if status.is_empty() { String::new() } else { format!(" [{}]", status.join(", ")) };
                
                println!("  - {}{}", model.name, status_str);
                println!("    {}", model.description.dimmed());
                println!("    Size: {} GB, Quality: {}/10", model.size_gb, model.quality_score);
            }
        }
        "/system" => {
            let specs = SystemSpecs::audit();
            println!("\n{}", "System Information:".bold());
            println!("  OS: {} {}", specs.os_name, specs.os_version);
            println!("  Memory: {} GB ({} GB available)", specs.total_memory_gb, specs.available_memory_gb);
            println!("  CPU: {} ({} cores, {} threads)", specs.cpu_brand, specs.cpu_cores, specs.cpu_threads);
        }
        "/theme" => {
            let themes = all_themes(custom_themes);
            if parts.len() == 1 {
                println!("\n{}", "Available Themes:".bold());
                for t in &themes { println!("  {}", t.name); }
            } else {
                let target = parts[1];
                if let Some(t) = themes.iter().find(|t| t.name == target) {
                    aide.config.active_theme = Some(t.name.clone());
                    aide.registry.save_config(&aide.config)?;
                    println!("Switched to theme: {}", t.name);
                } else {
                    println!("Theme not found: {}", target);
                }
            }
        }
        _ => println!("Unknown command."),
    }
    Ok(false)
}
