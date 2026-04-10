use std::path::Path;

use anyhow::{Context, Result};

pub fn run(config_path: &Path) -> Result<()> {
    let raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("cannot read config file: {}", config_path.display()))?;

    // Load through the full pipeline to prove validity, then print the raw YAML
    // so the user sees the file as-is (env var placeholders included for security).
    crate::config::load_from_path(config_path)
        .context("config file is invalid — showing raw content anyway")?;

    print!("{raw}");
    Ok(())
}
