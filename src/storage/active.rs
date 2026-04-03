use crate::storage::paths;
use anyhow::Result;
use std::fs;

pub fn read_active_id() -> Result<Option<u32>> {
    paths::ensure_layout()?;
    let path = paths::active_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.parse()?))
}

pub fn write_active_id(id: Option<u32>) -> Result<()> {
    paths::ensure_layout()?;
    let value = id.map(|value| value.to_string()).unwrap_or_default();
    fs::write(paths::active_file()?, value)?;
    Ok(())
}
