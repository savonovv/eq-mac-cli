use crate::storage::{index, presets};
use anyhow::Result;

pub fn run(selector: String, name: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    presets::rename_preset_file(&entry, &name)?;
    index::rename_entry(entry.id, &name)?;
    println!("renamed preset {} to {}", entry.id, name);
    Ok(())
}
