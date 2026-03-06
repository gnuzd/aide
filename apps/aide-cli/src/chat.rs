use crate::input::read_chat_line;
use crate::theme::{Theme, all_themes, get_theme};
use aide_core::Aide;
use aide_core::models::inference::InferenceEngine;
use aide_core::system::SystemSpecs;
use colored::*;
use crossterm::style::{
    Attribute, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType};
use std::io::{Write, stdout};
use std::sync::atomic::AtomicBool;

pub fn run_chat_loop(
    aide: &mut Aide,
    engine: InferenceEngine,
    session_id: &str,
) -> anyhow::Result<()> {
    println!("{}", "\n=== Aide Chat Mode ===".bold().cyan());
    println!("Type '/' for commands or 'exit' to quit.\n");

    let mut chat_history: Vec<String> = Vec::new();
    let stop = AtomicBool::new(false);
    let mut turn_number = 0;

    let system_prompt = aide
        .memory
        .get_profile_summary()
        .unwrap_or_else(|_| "You are Aide, a helpful assistant.".to_string());

    loop {
        let input = match read_chat_line(aide, &chat_history)? {
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

        // Standard theme fetch
        let mut custom_themes = Vec::new();
        for v in &aide.config.custom_themes {
            if let Ok(t) = serde_json::from_value::<Theme>(v.clone()) {
                custom_themes.push(t);
            }
        }
        let current_theme = get_theme(
            aide.config.active_theme.as_deref().unwrap_or("gruvbox"),
            &custom_themes,
        );

        // Handle slash commands
        if line.starts_with('/') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let cmd = parts[0];

            match cmd {
                "/help" => {
                    println!("\n{}", "Available Commands:".bold().yellow());
                    println!("  /clear          - Clear all memory and configuration");
                    println!("  /help           - Show this help message");
                    println!("  /memory         - Show what Aide knows about you");
                    println!("  /models         - List available and downloaded models");
                    println!("  /system         - Show system information");
                    println!("  /theme [name]   - List or switch color themes");
                }
                "/theme" => {
                    let themes = all_themes(&custom_themes);

                    if parts.len() == 1 {
                        println!("\n{}", "Available Themes:".bold().yellow());
                        let active = aide.config.active_theme.as_deref().unwrap_or("gruvbox");
                        for t in &themes {
                            let mark = if t.name == active { "*" } else { " " };
                            println!("  {} {}", mark.cyan(), t.name);
                        }
                        println!("\nUse '/theme <name>' to switch.");
                    } else {
                        let target = parts[1];
                        if let Some(t) = themes.iter().find(|t| t.name == target) {
                            aide.config.active_theme = Some(t.name.clone());
                            aide.registry.save_config(&aide.config)?;
                            println!("Switched to theme: {}", t.name.bold().cyan());
                        } else {
                            println!("Theme '{}' not found.", target);
                        }
                    }
                }
                "/system" => {
                    let specs = SystemSpecs::audit();
                    println!("\n{}", "System Information:".bold().yellow());
                    println!("  OS: {} {}", specs.os_name, specs.os_version);
                    println!(
                        "  RAM: {} GB ({} GB available)",
                        specs.total_memory_gb, specs.available_memory_gb
                    );
                    println!(
                        "  CPU: {} ({} cores, {} threads)",
                        specs.cpu_brand, specs.cpu_cores, specs.cpu_threads
                    );
                }
                "/models" => {
                    println!("\n{}", "Model Registry:".bold().yellow());
                    for model in &aide.registry.models {
                        let status = if aide
                            .registry
                            .base_path
                            .join("models")
                            .join(&model.filename)
                            .exists()
                        {
                            "[Downloaded]".green()
                        } else {
                            "[Available]".dimmed()
                        };
                        let active = if aide
                            .config
                            .active_model_path
                            .as_ref()
                            .map_or(false, |p| p.ends_with(&model.filename))
                        {
                            " (Active)".bold().cyan()
                        } else {
                            "".normal()
                        };
                        println!(
                            "  {} {} - {}{}",
                            status,
                            model.name.bold(),
                            model.description,
                            active
                        );
                    }
                }
                "/memory" => {
                    println!("\n{}", "User Profile (from Memory):".bold().yellow());
                    let (turns, sessions) = aide.memory.conversation_stats()?;
                    println!("  Total Turns: {}", turns);
                    println!("  Total Sessions: {}", sessions);

                    let summary = aide.memory.get_profile_summary()?;
                    println!("\n{}", "Profile Summary:".bold().cyan());
                    println!("  {}", summary);
                }
                "/clear" => {
                    print!("Are you sure you want to clear all memory and config? (y/N): ");
                    stdout().flush()?;
                    let mut confirm = String::new();
                    std::io::stdin().read_line(&mut confirm)?;
                    if confirm.trim().to_lowercase() == "y" {
                        aide.memory.clear_conversations()?;
                        aide.memory.clear_profile()?;
                        println!("{}", "Memory cleared.".green());
                    } else {
                        println!("Clear cancelled.");
                    }
                }
                _ => {
                    println!(
                        "Unknown command: {}. Type /help for a list of commands.",
                        line
                    );
                }
            }
            println!();
            continue;
        }

        // Aide response - Same line, no background
        print!("\n");
        crossterm::execute!(
            stdout(),
            SetForegroundColor(current_theme.h1_color()),
            SetAttribute(Attribute::Bold),
        )?;
        print!("Aide: ");
        crossterm::execute!(
            stdout(),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(current_theme.fg_color()),
        )?;
        stdout().flush()?;

        let mut response_full = String::new();

        engine.ask_stream(&line, &chat_history, 1024, &system_prompt, &stop, |token| {
            print!("{}", token);
            stdout().flush().unwrap();
            response_full.push_str(token);
        })?;

        print!("\n\n");
        stdout().flush()?;

        // Save to memory
        turn_number += 1;
        let _ = aide
            .memory
            .save_turn(session_id, turn_number, &line, &response_full);
        let _ = aide.memory.extract_and_learn(&line);

        chat_history.push(format!("User: {}", line));
        chat_history.push(format!("Assistant: {}", response_full));
    }

    Ok(())
}
