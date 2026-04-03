use crate::storage::paths;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::index::IndexEntry;

pub fn write_preset(entry: &IndexEntry, raw_text: &str) -> Result<()> {
    paths::ensure_layout()?;
    fs::write(preset_path(entry)?, raw_text)?;
    remove_legacy_numeric_file(entry.id)?;
    Ok(())
}

pub fn read_raw_preset(entry: &IndexEntry) -> Result<String> {
    let path = preset_path(entry)?;
    if path.exists() {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read preset {}", entry.name));
    }

    let legacy_path = legacy_preset_path(entry.id)?;
    let raw = fs::read_to_string(&legacy_path)
        .with_context(|| format!("failed to read preset {}", entry.name))?;

    fs::write(preset_path(entry)?, &raw)?;
    remove_legacy_numeric_file(entry.id)?;
    Ok(raw)
}

pub fn delete_preset(entry: &IndexEntry) -> Result<()> {
    let path = preset_path(entry)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    remove_legacy_numeric_file(entry.id)?;
    Ok(())
}

pub fn reindex_presets(entries: &[IndexEntry]) -> Result<Vec<IndexEntry>> {
    let rewritten = entries
        .iter()
        .enumerate()
        .map(|(offset, entry)| IndexEntry {
            id: (offset + 1) as u32,
            name: entry.name.clone(),
            created_at: entry.created_at.clone(),
        })
        .collect();
    Ok(rewritten)
}

pub fn rename_preset_file(entry: &IndexEntry, new_name: &str) -> Result<()> {
    paths::ensure_layout()?;
    let old_path = preset_path(entry)?;
    let new_path = named_preset_path(new_name)?;

    if old_path.exists() {
        fs::rename(old_path, new_path)?;
    } else {
        let legacy_path = legacy_preset_path(entry.id)?;
        if legacy_path.exists() {
            fs::rename(legacy_path, new_path)?;
        }
    }

    remove_legacy_numeric_file(entry.id)?;
    Ok(())
}

fn preset_path(entry: &IndexEntry) -> Result<PathBuf> {
    named_preset_path(&entry.name)
}

fn named_preset_path(name: &str) -> Result<PathBuf> {
    Ok(paths::presets_dir()?.join(format!("{}.txt", sanitize_filename(name))))
}

fn legacy_preset_path(id: u32) -> Result<PathBuf> {
    Ok(paths::presets_dir()?.join(format!("{id}.txt")))
}

fn remove_legacy_numeric_file(id: u32) -> Result<()> {
    let path = legacy_preset_path(id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    let mut previous_was_dash = false;

    for ch in name.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            previous_was_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !previous_was_dash {
            previous_was_dash = true;
            Some('-')
        } else {
            None
        };

        if let Some(ch) = normalized {
            sanitized.push(ch);
        }
    }

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "preset".to_string()
    } else {
        trimmed.to_string()
    }
}
