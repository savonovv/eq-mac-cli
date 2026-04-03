use crate::storage::paths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub input_device: Option<String>,
    pub output_device: Option<String>,
}

pub fn read_config() -> Result<Config> {
    paths::ensure_layout()?;
    let path = paths::config_file()?;
    if !path.exists() {
        return Ok(Config::default());
    }

    let raw = fs::read_to_string(path)?;
    let mut config = Config::default();
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("input=") {
            config.input_device = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("output=") {
            config.output_device = Some(value.trim().to_string());
        }
    }
    Ok(config)
}

pub fn write_config(config: &Config) -> Result<()> {
    paths::ensure_layout()?;
    let mut raw = String::new();
    if let Some(input) = &config.input_device {
        raw.push_str(&format!("input={input}\n"));
    }
    if let Some(output) = &config.output_device {
        raw.push_str(&format!("output={output}\n"));
    }
    fs::write(paths::config_file()?, raw)?;
    Ok(())
}
