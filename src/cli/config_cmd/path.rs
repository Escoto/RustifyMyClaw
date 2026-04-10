use anyhow::Result;

pub fn run() -> Result<()> {
    println!("{}", crate::config::dirs_path().display());
    Ok(())
}
