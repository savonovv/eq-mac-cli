use crate::cli::AudioCommand;
use crate::commands::daemon;
use crate::storage::config::{self, Config};
use anyhow::{bail, Result};
use cpal::traits::{DeviceTrait, HostTrait};

pub fn run(command: AudioCommand) -> Result<()> {
    match command {
        AudioCommand::List => list_devices(),
        AudioCommand::UseInput { name } => set_input(Some(name)),
        AudioCommand::UseOutput { name } => set_output(Some(name)),
        AudioCommand::Reset => reset(),
    }
}

pub fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    let current = config::read_config()?;

    println!("inputs:");
    for device in host.input_devices()? {
        let name = device.name()?;
        let marker = match current.input_device.as_deref() {
            Some(selected) if selected == name => "*",
            _ => " ",
        };
        println!("{marker} {name}");
    }

    println!();
    println!("outputs:");
    for device in host.output_devices()? {
        let name = device.name()?;
        let marker = match current.output_device.as_deref() {
            Some(selected) if selected == name => "*",
            _ => " ",
        };
        println!("{marker} {name}");
    }

    Ok(())
}

pub fn available_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    Ok(host
        .input_devices()?
        .filter_map(|device| device.name().ok())
        .collect())
}

pub fn available_output_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    Ok(host
        .output_devices()?
        .filter_map(|device| device.name().ok())
        .collect())
}

pub fn is_likely_system_audio_input(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("blackhole")
        || lower.contains("eqmac export")
        || lower.contains("soundflower")
        || lower.contains("loopback")
}

pub fn has_system_audio_input() -> Result<bool> {
    Ok(available_input_devices()?
        .iter()
        .any(|name| is_likely_system_audio_input(name)))
}

pub fn set_input(name: Option<String>) -> Result<()> {
    if let Some(name) = name.as_deref() {
        ensure_device_exists(true, name)?;
    }
    let mut current = config::read_config()?;
    current.input_device = name.clone();
    config::write_config(&current)?;
    restart_if_running()?;
    match name {
        Some(name) => println!("selected input device: {name}"),
        None => println!("selected input device: automatic"),
    }
    Ok(())
}

pub fn set_output(name: Option<String>) -> Result<()> {
    if let Some(name) = name.as_deref() {
        ensure_device_exists(false, name)?;
    }
    let mut current = config::read_config()?;
    current.output_device = name.clone();
    config::write_config(&current)?;
    restart_if_running()?;
    match name {
        Some(name) => println!("selected output device: {name}"),
        None => println!("selected output device: automatic"),
    }
    Ok(())
}

fn reset() -> Result<()> {
    config::write_config(&Config::default())?;
    restart_if_running()?;
    println!("audio device selection reset to automatic mode");
    Ok(())
}

fn ensure_device_exists(input: bool, target: &str) -> Result<()> {
    let host = cpal::default_host();
    let exists = if input {
        host.input_devices()?
            .any(|device| device.name().map(|name| name == target).unwrap_or(false))
    } else {
        host.output_devices()?
            .any(|device| device.name().map(|name| name == target).unwrap_or(false))
    };

    if !exists {
        let kind = if input { "input" } else { "output" };
        bail!("{kind} device not found: {target}");
    }

    Ok(())
}

fn restart_if_running() -> Result<()> {
    if crate::storage::daemon_state::is_running()? {
        daemon::run(crate::cli::DaemonCommand::Restart)?;
    }
    Ok(())
}
