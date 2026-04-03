use crate::eq::model::{ChannelFilters, Filter, FilterKind, Preset};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

pub fn parse_preset(input: &str, fallback_name: Option<String>) -> Result<Preset> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("preset input is empty");
    }

    if trimmed.lines().any(|line| line.starts_with("Preamp:")) {
        parse_autoeq(trimmed, fallback_name)
    } else {
        parse_native(trimmed, fallback_name)
    }
}

fn parse_autoeq(input: &str, fallback_name: Option<String>) -> Result<Preset> {
    let mut preamp_db = 0.0_f32;
    let mut filters = Vec::new();
    let mut channel_filters: BTreeMap<String, Vec<Filter>> = BTreeMap::new();
    let mut active_channels: Vec<String> = Vec::new();
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("Preamp:") {
            preamp_db = value.trim().trim_end_matches("dB").trim().parse()?;
            continue;
        }

        if let Some(value) = line.strip_prefix("Channel:") {
            active_channels = value
                .split(',')
                .map(str::trim)
                .filter(|channel| !channel.is_empty())
                .map(|channel| channel.to_uppercase())
                .collect();
            continue;
        }

        if !line.starts_with("Filter ") {
            continue;
        }

        if !line.contains(" ON ") {
            continue;
        }

        let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let tokens: Vec<_> = normalized.split_whitespace().collect();
        let kind_token = tokens
            .iter()
            .position(|token| *token == "ON")
            .and_then(|index| tokens.get(index + 1))
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing filter type in line: {line}"))?;

        let kind = match kind_token {
            "LS" | "LSC" => FilterKind::LowShelf,
            "HS" | "HSC" => FilterKind::HighShelf,
            "HP" => FilterKind::HighPass,
            "LP" => FilterKind::LowPass,
            "PK" => FilterKind::Peak,
            _ => bail!("unsupported filter type in line: {line}"),
        };

        let fc = extract_token(&normalized, "Fc", "Hz")?.parse()?;
        let gain = extract_optional_token(&normalized, "Gain", "dB")?
            .unwrap_or("0")
            .parse()?;
        let q = normalized
            .split("Q")
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("missing Q in line: {line}"))?
            .trim()
            .parse()?;

        let filter = Filter {
            enabled: normalized.contains(" ON "),
            kind,
            frequency_hz: fc,
            gain_db: gain,
            q,
        };

        if active_channels.is_empty() {
            filters.push(filter);
        } else {
            for channel in &active_channels {
                channel_filters
                    .entry(channel.clone())
                    .or_default()
                    .push(filter.clone());
            }
        }
    }

    Ok(Preset {
        name: fallback_name.unwrap_or_else(|| "Imported EQ".to_string()),
        preamp_db,
        filters,
        channel_filters: channel_filters
            .into_iter()
            .map(|(channel_name, filters)| ChannelFilters {
                channel_name,
                filters,
            })
            .collect(),
        original_text: input.to_string(),
    })
}

fn parse_native(input: &str, fallback_name: Option<String>) -> Result<Preset> {
    let mut name = fallback_name.unwrap_or_else(|| "Imported EQ".to_string());
    let mut preamp_db = 0.0_f32;
    let mut filters = Vec::new();

    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().to_string();
            continue;
        }
        if let Some(value) = line.strip_prefix("preamp:") {
            preamp_db = value.trim().parse()?;
            continue;
        }
        if let Some(value) = line.strip_prefix("filter:") {
            let parts: Vec<_> = value.split(',').map(|part| part.trim()).collect();
            if parts.len() != 4 {
                bail!("filter line must have 4 comma-separated values: {line}");
            }

            let kind = match parts[0].to_lowercase().as_str() {
                "lowshelf" => FilterKind::LowShelf,
                "highshelf" => FilterKind::HighShelf,
                "highpass" => FilterKind::HighPass,
                "lowpass" => FilterKind::LowPass,
                "peak" => FilterKind::Peak,
                other => bail!("unsupported filter kind {other}"),
            };

            filters.push(Filter {
                enabled: true,
                kind,
                frequency_hz: parts[1].parse()?,
                gain_db: parts[2].parse()?,
                q: parts[3].parse()?,
            });
        }
    }

    Ok(Preset {
        name,
        preamp_db,
        filters,
        channel_filters: Vec::new(),
        original_text: input.to_string(),
    })
}

fn extract_token<'a>(line: &'a str, start: &str, end: &str) -> Result<&'a str> {
    extract_optional_token(line, start, end)?
        .ok_or_else(|| anyhow::anyhow!("missing {start} in line: {line}"))
}

fn extract_optional_token<'a>(line: &'a str, start: &str, end: &str) -> Result<Option<&'a str>> {
    let after = line.split(start).nth(1);
    let Some(after) = after else {
        return Ok(None);
    };
    let value = after
        .split(end)
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing {end} in line: {line}"))?;
    Ok(Some(value.trim()))
}

#[cfg(test)]
mod tests {
    use super::parse_preset;
    use crate::eq::model::FilterKind;

    #[test]
    fn parses_extended_equalizer_apo_filters() {
        let preset = parse_preset(
            "Preamp: -3 dB\nFilter 1: ON HS Fc 8000 Hz Gain 4 dB Q 0.707\nFilter 2: ON HP Fc 80 Hz Q 0.707\nFilter 3: ON LP Fc 16000 Hz Q 0.707",
            Some("Test".to_string()),
        )
        .unwrap();

        assert_eq!(preset.filters.len(), 3);
        assert!(preset.channel_filters.is_empty());
        assert!(matches!(preset.filters[0].kind, FilterKind::HighShelf));
        assert_eq!(preset.filters[0].gain_db, 4.0);
        assert!(matches!(preset.filters[1].kind, FilterKind::HighPass));
        assert_eq!(preset.filters[1].gain_db, 0.0);
        assert!(matches!(preset.filters[2].kind, FilterKind::LowPass));
        assert_eq!(preset.filters[2].gain_db, 0.0);
    }

    #[test]
    fn parses_channel_blocks() {
        let preset = parse_preset(
            "Preamp: -1 dB\nChannel: L\nFilter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41\nChannel: R\nFilter 2: ON HS Fc 8000 Hz Gain 2 dB Q 0.70",
            Some("Ch".to_string()),
        )
        .unwrap();

        assert!(preset.filters.is_empty());
        assert_eq!(preset.channel_filters.len(), 2);
        assert_eq!(preset.channel_filters[0].channel_name, "L");
        assert_eq!(preset.channel_filters[0].filters.len(), 1);
        assert_eq!(preset.channel_filters[1].channel_name, "R");
        assert_eq!(preset.channel_filters[1].filters.len(), 1);
    }
}
