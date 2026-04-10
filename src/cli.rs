use std::path::PathBuf;

use clap::Parser;

/// Daemon bridging messaging platforms to local AI CLI tools.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Path to config file (default: platform-specific location).
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Log level override. Priority: this flag > RUST_LOG env var > "info".
    #[arg(short, long)]
    pub log_level: Option<String>,

    /// Validate the config file and exit without starting the daemon.
    #[arg(long)]
    pub validate: bool,
}
