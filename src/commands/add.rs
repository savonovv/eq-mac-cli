use crate::eq::{parser, validate};
use crate::storage::{index, presets};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::PathBuf;

pub fn run(file: Option<PathBuf>, text: Option<String>, name: String) -> Result<()> {
    let raw = match (file, text) {
        (Some(path), None) => fs::read_to_string(&path)
            .with_context(|| format!("failed to read preset file {}", path.display()))?,
        (None, Some(text)) => text,
        _ => bail!("provide exactly one of --file or --text"),
    };

    let preset = parser::parse_preset(&raw, Some(name.clone()))?;
    validate::validate_preset(&preset)?;
    let next_id = index::next_id()?;
    presets::write_preset(next_id, &preset.original_text)?;
    index::append_entry(next_id, &name)?;
    println!("saved preset {next_id}: {name}");
    Ok(())
}
