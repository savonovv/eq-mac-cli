use crate::commands::daemon;
use crate::storage::{active, daemon_state, index};
use anyhow::{Context, Result};
use std::fs;

pub fn run(selector: String) -> Result<()> {
    let entry = index::resolve_selector(&selector)?;
    active::write_active_id(Some(entry.id))?;
    let started = daemon::ensure_started_with_state()?;
    if !started {
        daemon::notify_reload().with_context(|| {
            let log_tail = fs::read_to_string(daemon_state::log_path().unwrap_or_default())
                .ok()
                .and_then(|content| content.lines().last().map(str::to_string))
                .unwrap_or_else(|| "no daemon log output".to_string());
            format!("failed to notify daemon; last daemon log line: {log_tail}")
        })?;
    }
    println!("enabled preset {}: {}", entry.id, entry.name);
    Ok(())
}
