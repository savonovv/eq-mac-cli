use crate::storage::paths;
use anyhow::{Context, Result};
use std::fs;

pub fn write_preset(id: u32, raw_text: &str) -> Result<()> {
    paths::ensure_layout()?;
    fs::write(preset_path(id)?, raw_text)?;
    Ok(())
}

pub fn read_raw_preset(id: u32) -> Result<String> {
    fs::read_to_string(preset_path(id)?).with_context(|| format!("failed to read preset {id}"))
}

pub fn delete_preset(id: u32) -> Result<()> {
    let path = preset_path(id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn preset_path(id: u32) -> Result<std::path::PathBuf> {
    Ok(paths::presets_dir()?.join(format!("{id}.txt")))
}
