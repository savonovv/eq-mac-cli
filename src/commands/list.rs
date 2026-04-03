use crate::storage::{active, index};
use anyhow::Result;

pub fn run() -> Result<()> {
    let entries = index::read_entries()?;
    let active_id = active::read_active_id()?;

    if entries.is_empty() {
        println!("no presets saved");
        return Ok(());
    }

    for entry in entries {
        let marker = if Some(entry.id) == active_id {
            "*"
        } else {
            " "
        };
        println!("{marker} {} {}", entry.id, entry.name);
    }

    Ok(())
}
