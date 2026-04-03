use crate::storage::index;
use anyhow::Result;

pub fn run(selector: String, name: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    index::rename_entry(entry.id, &name)?;
    println!("renamed preset {} to {}", entry.id, name);
    Ok(())
}
