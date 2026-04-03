use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub fn data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory not found"))?;
    Ok(home.join(".local/share/eq-mac-cli"))
}

pub fn presets_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("presets"))
}

pub fn runtime_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("runtime"))
}

pub fn index_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("index.txt"))
}

pub fn active_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("active.txt"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("config.txt"))
}

pub fn ensure_layout() -> Result<()> {
    fs::create_dir_all(data_dir()?)?;
    fs::create_dir_all(presets_dir()?)?;
    fs::create_dir_all(runtime_dir()?)?;
    Ok(())
}
