use crate::storage::paths;
use anyhow::{Result, bail};
use chrono::Utc;
use std::fs;

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub id: u32,
    pub name: String,
    pub created_at: String,
}

pub fn read_entries() -> Result<Vec<IndexEntry>> {
    paths::ensure_layout()?;
    let path = paths::index_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)?;
    let mut entries = Vec::new();
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let parts: Vec<_> = line.split('|').collect();
        if parts.len() != 3 {
            continue;
        }
        entries.push(IndexEntry {
            id: parts[0].parse()?,
            name: parts[1].to_string(),
            created_at: parts[2].to_string(),
        });
    }
    Ok(entries)
}

pub fn append_entry(id: u32, name: &str) -> Result<()> {
    let mut entries = read_entries()?;
    entries.push(IndexEntry {
        id,
        name: name.to_string(),
        created_at: Utc::now().to_rfc3339(),
    });
    write_entries(&entries)
}

pub fn write_entries(entries: &[IndexEntry]) -> Result<()> {
    paths::ensure_layout()?;
    let mut raw = String::new();
    for entry in entries {
        raw.push_str(&format!(
            "{}|{}|{}\n",
            entry.id, entry.name, entry.created_at
        ));
    }
    fs::write(paths::index_file()?, raw)?;
    Ok(())
}

pub fn next_id() -> Result<u32> {
    let entries = read_entries()?;
    Ok(entries.last().map(|entry| entry.id + 1).unwrap_or(1))
}

pub fn resolve_selector(selector: &str) -> Result<IndexEntry> {
    if let Ok(id) = selector.parse::<u32>() {
        return resolve_id(id);
    }

    let entries = read_entries()?;
    let matches: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.name == selector)
        .collect();

    match matches.as_slice() {
        [entry] => Ok(entry.clone()),
        [] => bail!("preset not found: {selector}"),
        _ => bail!("multiple presets matched name: {selector}; use numeric id"),
    }
}

pub fn resolve_id(id: u32) -> Result<IndexEntry> {
    read_entries()?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow::anyhow!("preset not found: {id}"))
}

pub fn remove_entry(id: u32) -> Result<()> {
    let mut entries = read_entries()?;
    entries.retain(|entry| entry.id != id);
    write_entries(&entries)
}

pub fn rename_entry(id: u32, new_name: &str) -> Result<()> {
    let mut entries = read_entries()?;
    let mut found = false;
    for entry in &mut entries {
        if entry.id == id {
            entry.name = new_name.to_string();
            found = true;
            break;
        }
    }
    if !found {
        bail!("preset not found: {id}");
    }
    write_entries(&entries)
}
