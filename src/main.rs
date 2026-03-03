mod system;
mod models;
mod memory;
mod theme;
mod ui;
mod cli;

use clap::Parser;
use cli::{Cli, Commands};
use colored::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Chat) => {
            cli::run_chat_loop()?;
        }
        Some(Commands::Setup) => {
            cli::run_setup().await?;
        }
        Some(Commands::System) => {
            cli::show_system_info();
        }
        Some(Commands::Models) => {
            cli::list_models().await?;
        }
        Some(Commands::Clear) => {
            if cli::run_clear()? {
                println!("\n{}", "Re-running setup...".cyan().bold());
                cli::run_setup().await?;
            }
        }
        Some(Commands::Theme) => {
            cli::run_theme()?;
        }
        None => {
            let registry = models::ModelRegistry::new();
            let config = registry.load_config()?;
            if config.active_model_path.is_some() {
                cli::run_chat_loop()?;
            } else {
                println!("{}", "No active model found. Starting setup...".cyan().bold());
                cli::run_setup().await?;
            }
        }
    }

    Ok(())
}
