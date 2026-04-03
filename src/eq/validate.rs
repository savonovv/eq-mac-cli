use crate::eq::model::Preset;
use anyhow::{bail, Result};

pub fn validate_preset(preset: &Preset) -> Result<()> {
    if preset.filters.is_empty()
        && preset
            .channel_filters
            .iter()
            .all(|channel| channel.filters.is_empty())
    {
        bail!("preset must contain at least one filter");
    }

    for filter in &preset.filters {
        validate_filter(filter)?;
    }

    for channel in &preset.channel_filters {
        for filter in &channel.filters {
            validate_filter(filter)?;
        }
    }

    Ok(())
}

fn validate_filter(filter: &crate::eq::model::Filter) -> Result<()> {
    if filter.frequency_hz <= 0.0 {
        bail!("filter frequency must be greater than 0 Hz");
    }
    if filter.q <= 0.0 {
        bail!("filter Q must be greater than 0");
    }
    Ok(())
}
