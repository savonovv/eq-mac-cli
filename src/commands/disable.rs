use crate::commands::daemon;
use crate::storage::active;
use anyhow::Result;

pub fn run() -> Result<()> {
    active::write_active_id(None)?;
    let started = daemon::ensure_started_with_state()?;
    if !started {
        daemon::notify_reload()?;
    }
    println!("eq disabled; daemon left running in bypass mode");
    Ok(())
}
