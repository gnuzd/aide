use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select};
use crate::system::SystemSpecs;
use crate::models::ModelRegistry;

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
}

pub fn run_setup() {
    println!("{}", "=== Aide Setup Wizard ===".bold().cyan());
    
    let specs = SystemSpecs::audit();
    println!("\n{}", "1. Auditing Hardware...".bold());
    println!("- OS: {} {}", specs.os_name, specs.os_version);
    println!("- CPU: {} ({} cores / {} threads)", specs.cpu_brand, specs.cpu_cores, specs.cpu_threads);
    println!("- RAM: {} GB total ({} GB available)", specs.total_memory_gb, specs.available_memory_gb);

    let (compatible, warnings) = specs.check_compatibility();
    if !compatible {
        println!("\n{}", "⚠️  Minimum hardware requirements not met!".red().bold());
    }

    for warning in warnings {
        println!("  - {}", warning.yellow());
    }

    println!("\n{}", "2. Select Main Model...".bold());
    let registry = ModelRegistry::new();
    let compatible_models = registry.get_compatible_models(&specs);

    if compatible_models.is_empty() {
        println!("{}", "No suitable local models found for your hardware.".red());
    } else {
        let items: Vec<String> = compatible_models
            .iter()
            .map(|m| format!("{} - {} ({} GB RAM min)", m.name, m.description, m.min_ram_gb))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Pick a model to download as your main model")
            .items(&items)
            .default(0)
            .interact()
            .unwrap();

        let selected_model = compatible_models[selection];
        println!("\nYou selected: {}", selected_model.name.green().bold());
        println!("We will download this in Phase 2 implementation.");
    }

    println!("\n{}", "Setup complete! Starting chat mode...".green());
    let _ = run_chat_loop();
}

pub fn run_chat_loop() -> anyhow::Result<()> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    println!("{}", "\n=== Aide Chat Mode ===".bold().cyan());
    println!("Type 'exit' or 'quit' to end the session.\n");

    let mut rl = DefaultEditor::new()?;
    
    loop {
        let readline = rl.readline(&format!("{} > ", "You".green().bold()));
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() { continue; }
                if line == "exit" || line == "quit" { break; }
                
                let _ = rl.add_history_entry(line);

                // Placeholder for Phase 2: Local AI Inference
                println!("{}: {}", "Aide".cyan().bold(), "Hello! I am your local AI assistant.");
                println!("           (In Phase 2, I'll process your real requests locally.)\n");
            },
            Err(ReadlineError::Interrupted) => {
                println!("\nSession ended.");
                break;
            },
            Err(ReadlineError::Eof) => {
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
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
    for model in registry.models {
        println!("{:<20} | {:<10?} | {:<8} GB", model.name, model.model_type, model.min_ram_gb);
    }
}
