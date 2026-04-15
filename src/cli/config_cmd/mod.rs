mod allow_path;
mod dotted_path;
mod get;
mod init;
mod path;
mod set;
mod show;
mod validate;

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print the resolved configuration (env vars interpolated).
    Show,
    /// Validate the config file and exit.
    Validate,
    /// Print the default config file path.
    Path,
    /// Generate a starter config.yaml.
    Init {
        /// Write template to a file. Ex. usr/configs/rustifymyclaw.yaml
        #[arg(short, long, conflicts_with = "dir")]
        file: Option<std::path::PathBuf>,
        /// Write config.yaml inside a directory. Ex. usr/configs/
        #[arg(short, long, conflicts_with = "file")]
        dir: Option<std::path::PathBuf>,
    },
    /// Read a single config value by dotted path (e.g. output.max_message_chars).
    Get {
        /// Dotted path to the value (e.g. `workspaces[0].backend`).
        key: String,
    },
    /// Write a single config value by dotted path.
    Set {
        /// Dotted path to the value (e.g. output.max_message_chars).
        key: String,
        /// New value (auto-detected as int, bool, or string).
        value: String,
    },
    /// Grant the systemd service read-write access to a workspace directory (Linux only, requires sudo).
    AllowPath {
        /// Filesystem path to allow (e.g. /home/user/projects/my-project).
        path: std::path::PathBuf,
    },
}

pub fn run(action: &ConfigAction, config_path: &Path) -> Result<()> {
    match action {
        ConfigAction::Show => show::run(config_path),
        ConfigAction::Validate => validate::run(config_path),
        ConfigAction::Path => path::run(config_path),
        ConfigAction::Init { file, dir } => init::run(file.as_deref(), dir.as_deref(), config_path),
        ConfigAction::Get { key } => get::run(config_path, key),
        ConfigAction::Set { key, value } => set::run(config_path, key, value),
        ConfigAction::AllowPath { path } => allow_path::run(path),
    }
}
