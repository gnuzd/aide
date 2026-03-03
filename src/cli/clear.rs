use crate::memory::MemoryStore;
use crate::models::ModelRegistry;
use colored::*;
use dialoguer::{Confirm, MultiSelect, theme::ColorfulTheme};
use std::path::PathBuf;

pub fn run_clear() -> anyhow::Result<bool> {
    println!("{}", "=== Aide: Clear Data ===".bold().cyan());

    let aide_dir = dirs::home_dir()
        .map(|h| h.join(".aide"))
        .unwrap_or_else(|| PathBuf::from(".aide"));

    if !aide_dir.exists() {
        println!("Nothing to clear — ~/.aide/ does not exist.");
        return Ok(false);
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

    // ... (rest of the action gathering logic is same)

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
            return Ok(false);
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
        return Ok(false);
    }

    println!();

    let mut needs_setup = false;

    // "Everything" short-circuits all other selections
    if selections.iter().any(|&i| matches!(actions[i], Action::Everything)) {
        std::fs::remove_dir_all(&aide_dir)?;
        println!("{}", "Deleted ~/.aide/ — Aide has been completely reset.".green().bold());
        return Ok(true);
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
                        needs_setup = true;
                    }
                }
            }
            Action::Config => {
                std::fs::remove_file(&config_path)?;
                println!("Cleared config.");
                needs_setup = true;
            }
            Action::Everything => unreachable!(),
        }
    }

    println!("\n{}", "Done.".green().bold());
    Ok(needs_setup)
}
