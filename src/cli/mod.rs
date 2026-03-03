mod chat;
mod clear;
mod input;
mod models_cmd;
mod setup;
mod theme_cmd;

use clap::{Parser, Subcommand};

pub use chat::run_chat_loop;
pub use clear::run_clear;
pub use input::read_chat_line;
pub use models_cmd::{list_models, show_system_info};
pub use setup::run_setup;
pub use theme_cmd::run_theme;

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
