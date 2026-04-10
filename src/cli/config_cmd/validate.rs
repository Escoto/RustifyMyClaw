use std::path::Path;

use anyhow::Result;

pub fn run(config_path: &Path) -> Result<()> {
    crate::config::load_from_path(config_path)?;
    println!("configuration is valid: {}", config_path.display());
    Ok(())
}
