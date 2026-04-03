use crate::storage::{active, index, presets};
use anyhow::Result;

pub fn run(selector: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    presets::delete_preset(entry.id)?;
    index::remove_entry(entry.id)?;
    if active::read_active_id()? == Some(entry.id) {
        active::write_active_id(None)?;
    }
    println!("deleted preset {}: {}", entry.id, entry.name);
    Ok(())
}
