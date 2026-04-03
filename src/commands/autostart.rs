use crate::cli::AutostartCommand;
use crate::storage::{autostart, paths};
use anyhow::{Context, Result, bail};
use std::fs;
use std::process::Command;

pub fn run(command: AutostartCommand) -> Result<()> {
    match command {
        AutostartCommand::Enable => enable(),
        AutostartCommand::Disable => disable(),
    }
}

fn enable() -> Result<()> {
    let plist_path = autostart::plist_path()?;
    let daemon_path = std::env::current_exe()?.with_file_name("eqmacd");
    if !daemon_path.exists() {
        bail!(
            "eqmacd binary not found next to eqcli at {}",
            daemon_path.display()
        );
    }

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let plist = autostart::render_plist(&daemon_path, &paths::data_dir()?);
    fs::write(&plist_path, plist)
        .with_context(|| format!("failed to write {}", plist_path.display()))?;

    let _ = Command::new("launchctl")
        .args(["unload", plist_path.to_string_lossy().as_ref()])
        .status();

    let status = Command::new("launchctl")
        .args(["load", plist_path.to_string_lossy().as_ref()])
        .status()?;

    if !status.success() {
        bail!("failed to load launchd plist");
    }

    println!("autostart enabled");
    Ok(())
}

fn disable() -> Result<()> {
    let plist_path = autostart::plist_path()?;
    if plist_path.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", plist_path.to_string_lossy().as_ref()])
            .status();
        fs::remove_file(&plist_path)?;
    }
    println!("autostart disabled");
    Ok(())
}
