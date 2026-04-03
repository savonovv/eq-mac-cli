use crate::cli::DaemonCommand;
use crate::storage::daemon_state;
use anyhow::{bail, Context, Result};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn run(command: DaemonCommand) -> Result<()> {
    match command {
        DaemonCommand::Start => start(),
        DaemonCommand::Stop => stop(),
        DaemonCommand::Restart => restart(),
    }
}

pub fn ensure_started() -> Result<()> {
    if daemon_state::is_running()? {
        return Ok(());
    }
    cleanup_stale_runtime()?;
    start()
}

pub fn ensure_started_with_state() -> Result<bool> {
    if daemon_state::is_running()? {
        return Ok(false);
    }
    cleanup_stale_runtime()?;
    start()?;
    Ok(true)
}

pub fn notify_reload() -> Result<()> {
    send_command_with_retry("reload")
}

fn start() -> Result<()> {
    let daemon_path = std::env::current_exe()?.with_file_name("eqmacd");
    if !daemon_path.exists() {
        bail!(
            "eqmacd binary not found next to eqcli at {}",
            daemon_path.display()
        );
    }

    if daemon_state::is_running()? {
        println!("daemon already running");
        return Ok(());
    }

    if let Some(pid) = find_running_daemon_pid(&daemon_path)? {
        daemon_state::write_pid(pid)?;
        wait_until_ready()?;
        println!("daemon already running");
        return Ok(());
    }

    cleanup_stale_runtime()?;

    if let Some(pid) = find_running_daemon_pid(&daemon_path)? {
        daemon_state::write_pid(pid)?;
        wait_until_ready()?;
        println!("daemon already running");
        return Ok(());
    }

    let log_path = daemon_state::log_path()?;
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .with_context(|| format!("failed to open daemon log {}", log_path.display()))?;
    let log_err = log.try_clone()?;

    let child = Command::new(&daemon_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .with_context(|| format!("failed to start {}", daemon_path.display()))?;

    daemon_state::write_pid(child.id())?;
    wait_until_ready()?;
    println!("daemon started");
    Ok(())
}

fn stop() -> Result<()> {
    if !daemon_state::is_running()? {
        println!("daemon not running");
        return Ok(());
    }

    send_command_with_retry("stop")?;
    println!("daemon stop requested");
    Ok(())
}

fn restart() -> Result<()> {
    if daemon_state::is_running()? {
        notify_reload()?;
        println!("daemon reloaded");
        return Ok(());
    }

    start()
}

fn send_command_with_retry(command: &str) -> Result<()> {
    let mut last_err = None;
    for _ in 0..20 {
        match send_command(command) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("failed to send daemon command")))
}

fn wait_until_ready() -> Result<()> {
    for _ in 0..30 {
        if daemon_state::is_running()? && daemon_state::socket_path()?.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    bail!(
        "daemon did not become ready; check {}",
        daemon_state::log_path()?.display()
    )
}

fn cleanup_stale_runtime() -> Result<()> {
    let socket = daemon_state::socket_path()?;
    if socket.exists() && !daemon_state::is_running()? {
        let _ = fs::remove_file(&socket);
    }

    let pid = daemon_state::pid_path()?;
    if pid.exists() && !daemon_state::is_running()? {
        let _ = fs::remove_file(&pid);
    }

    Ok(())
}

fn send_command(command: &str) -> Result<()> {
    let socket = daemon_state::socket_path()?;
    let mut stream = UnixStream::connect(&socket)
        .with_context(|| format!("failed to connect to {}", socket.display()))?;
    stream.write_all(command.as_bytes())?;
    Ok(())
}

fn find_running_daemon_pid(daemon_path: &Path) -> Result<Option<u32>> {
    let output = Command::new("pgrep")
        .args(["-f", &daemon_path.display().to_string()])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let pid = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::parse)
        .transpose()?;

    Ok(pid)
}
