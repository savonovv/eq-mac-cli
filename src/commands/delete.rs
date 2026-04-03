use crate::storage::{active, index, presets};
use anyhow::Result;

pub fn run(selector: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    presets::delete_preset(&entry)?;

    let active_id = active::read_active_id()?;
    let remaining_entries: Vec<_> = index::read_entries()?
        .into_iter()
        .filter(|candidate| candidate.id != entry.id)
        .collect();
    let reindexed_entries = presets::reindex_presets(&remaining_entries)?;
    index::write_entries(&reindexed_entries)?;

    let next_active_id = match active_id {
        Some(id) if id == entry.id => None,
        Some(id) if id > entry.id => Some(id - 1),
        other => other,
    };
    active::write_active_id(next_active_id)?;

    println!("deleted preset {}: {}", entry.id, entry.name);
    Ok(())
}
