mod config_cmd;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config;

/// Daemon bridging messaging platforms to local AI CLI tools.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Path to config file (default: platform-specific location).
    #[arg(short = 'f', long = "config-file", global = true)]
    pub config_file: Option<PathBuf>,

    /// Log level override. Priority: this flag > RUST_LOG env var > "info".
    #[arg(short, long)]
    pub log_level: Option<String>,

    /// Validate the config file and exit without starting the daemon.
    #[arg(long)]
    pub validate: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Inspect and modify configuration.
    Config {
        #[command(subcommand)]
        action: config_cmd::ConfigAction,
    },
}

/// Run a subcommand if present. Returns `true` if a subcommand was handled
/// (caller should exit), `false` if the daemon should start normally.
pub fn run_command(cli: &Cli) -> Result<bool> {
    let config_path = config::resolve_path(cli.config_file.clone());

    if cli.validate {
        config::load_from_path(&config_path)?;
        println!("configuration is valid: {}", config_path.display());
        return Ok(true);
    }

    match &cli.command {
        Some(Command::Config { action }) => {
            config_cmd::run(action, &config_path)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
