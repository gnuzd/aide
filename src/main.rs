mod system;
mod models;
mod memory;
mod theme;
mod ui;
mod cli;

use clap::Parser;
use cli::{Cli, Commands};

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
            cli::list_models();
        }
        Some(Commands::Clear) => {
            cli::run_clear()?;
        }
        Some(Commands::Theme) => {
            cli::run_theme()?;
        }
        None => {
            println!("Welcome to Aide! Use `aide setup` to get started or `aide chat` to begin.");
        }
    }

    Ok(())
}
