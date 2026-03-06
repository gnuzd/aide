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
    
    loop {
        let mut aide = Aide::new()?;
        aide.init().await?;

        let session_id = aide.generate_session_id();

        match cli.command {
            Some(Commands::Chat) | None => {
                let engine = aide.create_inference_engine()?;
                chat::run_chat_loop(&mut aide, engine, &session_id)?;
                // After chat ends, we check if it was a normal exit or reset exit.
                // For now, if chat loop finishes normally (not an error), break.
                // But wait — if it's a reset, chat returns Ok(()).
                // We need to differentiate or always check if config exists.
                if !aide.registry.get_config_path().exists() {
                    // Reset was performed
                    continue;
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
