mod chat;
mod cli;
mod input;
mod theme;

use clap::Parser;
use cli::{Cli, Commands};
use aide_core::Aide;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    let mut aide = Aide::new()?;
    aide.init().await?;

    let session_id = aide.generate_session_id();

    match cli.command {
        Some(Commands::Chat) => {
            let engine = aide.create_inference_engine()?;
            chat::run_chat_loop(&mut aide, engine, &session_id)?;
        }
        None => {
            let engine = aide.create_inference_engine()?;
            chat::run_chat_loop(&mut aide, engine, &session_id)?;
        }
    }

    Ok(())
}
