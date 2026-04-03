use crate::storage::{index, presets};
use anyhow::Result;

pub fn run(selector: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    let raw = presets::read_raw_preset(entry.id)?;
    println!("id: {}", entry.id);
    println!("name: {}", entry.name);
    println!();
    println!("{raw}");
    Ok(())
}
