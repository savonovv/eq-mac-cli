use crate::storage::paths;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn pid_path() -> Result<PathBuf> {
    Ok(paths::runtime_dir()?.join("daemon.pid"))
}

pub fn socket_path() -> Result<PathBuf> {
    Ok(paths::runtime_dir()?.join("daemon.sock"))
}

pub fn log_path() -> Result<PathBuf> {
    Ok(paths::runtime_dir()?.join("daemon.log"))
}

pub fn write_pid(pid: u32) -> Result<()> {
    paths::ensure_layout()?;
    fs::write(pid_path()?, pid.to_string())?;
    Ok(())
}

pub fn read_pid() -> Result<Option<u32>> {
    let path = pid_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    Ok(Some(raw.trim().parse()?))
}

pub fn clear_pid() -> Result<()> {
    let path = pid_path()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn is_running() -> Result<bool> {
    let Some(pid) = read_pid()? else {
        return Ok(false);
    };

    let status = Command::new("kill").args(["-0", &pid.to_string()]).status();
    Ok(status.map(|s| s.success()).unwrap_or(false))
}
