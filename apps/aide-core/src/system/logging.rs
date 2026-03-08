use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use std::path::PathBuf;

pub fn init_logging(base_path: &PathBuf) -> anyhow::Result<()> {
    let log_dir = base_path.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "aide.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // We need to leak the guard to keep it alive for the duration of the program
    // In a CLI, this is acceptable.
    Box::leak(Box::new(_guard));

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr).with_filter(EnvFilter::new("error")))
        .init();

    tracing::info!("Logging initialized. Logs stored in {:?}", log_dir);
    Ok(())
}
