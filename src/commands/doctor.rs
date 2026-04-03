use crate::commands::audio;
use crate::storage::{autostart, config, daemon_state};
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use std::process::Command;

pub fn run() -> Result<()> {
    let brew = Command::new("which").arg("brew").output();
    let brew_ok = brew.map(|o| o.status.success()).unwrap_or(false);
    println!("brew installed: {}", brew_ok);

    let blackhole_ok = if brew_ok {
        Command::new("brew")
            .args(["list", "--cask", "blackhole-2ch"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };
    println!("blackhole-2ch installed: {}", blackhole_ok);
    println!("daemon running: {}", daemon_state::is_running()?);
    println!("autostart enabled: {}", autostart::is_enabled()?);

    let config = config::read_config()?;
    println!(
        "configured input device: {}",
        config.input_device.as_deref().unwrap_or("automatic")
    );
    println!(
        "configured output device: {}",
        config.output_device.as_deref().unwrap_or("automatic")
    );

    let host = cpal::default_host();
    let inputs: Vec<String> = host
        .input_devices()?
        .filter_map(|device| device.name().ok())
        .collect();
    let outputs: Vec<String> = host
        .output_devices()?
        .filter_map(|device| device.name().ok())
        .collect();

    if let Some(selected) = &config.input_device {
        println!(
            "configured input available: {}",
            inputs.iter().any(|name| name == selected)
        );
    }
    if let Some(selected) = &config.output_device {
        println!(
            "configured output available: {}",
            outputs.iter().any(|name| name == selected)
        );
    }

    let effective_input = config.input_device.as_deref();
    let routing_ok = match effective_input {
        Some(name) => audio::is_likely_system_audio_input(name),
        None => audio::has_system_audio_input()?,
    };
    if !routing_ok {
        println!(
            "warning: selected input is not a known loopback/system-audio device; current EQ path is likely microphone or line-in, not app playback"
        );
    }

    if blackhole_ok {
        println!("note: blackhole-2ch changes may require logout/login or reboot");
    } else {
        println!("note: install blackhole-2ch for system-output routing");
    }

    Ok(())
}
