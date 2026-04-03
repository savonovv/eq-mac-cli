use crate::eq::model::Preset;
use anyhow::{Result, bail};

pub fn validate_preset(preset: &Preset) -> Result<()> {
    if preset.filters.is_empty() {
        bail!("preset must contain at least one filter");
    }

    for filter in &preset.filters {
        if filter.frequency_hz <= 0.0 {
            bail!("filter frequency must be greater than 0 Hz");
        }
        if filter.q <= 0.0 {
            bail!("filter Q must be greater than 0");
        }
    }

    Ok(())
}
