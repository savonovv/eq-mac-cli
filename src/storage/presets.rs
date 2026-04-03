use crate::storage::paths;
use anyhow::{Context, Result};
use std::fs;

use super::index::IndexEntry;

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

pub fn reindex_presets(entries: &[IndexEntry]) -> Result<Vec<IndexEntry>> {
    paths::ensure_layout()?;
    let presets_dir = paths::presets_dir()?;
    let temp_dir = presets_dir.join(".reindex_tmp");

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let mut rewritten = Vec::with_capacity(entries.len());
    for (offset, entry) in entries.iter().enumerate() {
        let new_id = (offset + 1) as u32;
        let raw = read_raw_preset(entry.id)?;
        fs::write(temp_dir.join(format!("{new_id}.txt")), raw)?;
        rewritten.push(IndexEntry {
            id: new_id,
            name: entry.name.clone(),
            created_at: entry.created_at.clone(),
        });
    }

    if presets_dir.exists() {
        for child in fs::read_dir(&presets_dir)? {
            let child = child?;
            let path = child.path();
            if path == temp_dir {
                continue;
            }
            if path.is_file() {
                fs::remove_file(path)?;
            }
        }
    }

    for entry in &rewritten {
        fs::rename(
            temp_dir.join(format!("{}.txt", entry.id)),
            presets_dir.join(format!("{}.txt", entry.id)),
        )?;
    }
    fs::remove_dir_all(temp_dir)?;

    Ok(rewritten)
}

fn preset_path(id: u32) -> Result<std::path::PathBuf> {
    Ok(paths::presets_dir()?.join(format!("{id}.txt")))
}
