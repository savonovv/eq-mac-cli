use crate::commands::audio;
use crate::storage::{active, autostart, config, daemon_state, index};
use anyhow::Result;

pub fn run() -> Result<()> {
    let active_id = active::read_active_id()?;
    match active_id {
        Some(id) => {
            let entry = index::resolve_id(id)?;
            println!("active preset: {} ({})", entry.id, entry.name);
        }
        None => println!("active preset: disabled"),
    }

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
    println!("control socket: {}", daemon_state::socket_path()?.display());
    println!("daemon log: {}", daemon_state::log_path()?.display());

    let effective_input = config.input_device.as_deref();
    let routing_ok = match effective_input {
        Some(name) => audio::is_likely_system_audio_input(name),
        None => audio::has_system_audio_input()?,
    };
    if !routing_ok {
        println!(
            "warning: current input is not a known loopback/system-audio device; EQ may not affect app or music playback"
        );
    }
    Ok(())
}
