use crate::eq::model::{Filter, FilterKind, Preset};
use anyhow::{Result, bail};

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
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("Preamp:") {
            preamp_db = value.trim().trim_end_matches("dB").trim().parse()?;
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
            "PK" => FilterKind::Peak,
            _ => bail!("unsupported filter type in line: {line}"),
        };

        let fc = extract_token(&normalized, "Fc", "Hz")?.parse()?;
        let gain = extract_token(&normalized, "Gain", "dB")?.parse()?;
        let q = normalized
            .split("Q")
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("missing Q in line: {line}"))?
            .trim()
            .parse()?;

        filters.push(Filter {
            kind,
            frequency_hz: fc,
            gain_db: gain,
            q,
        });
    }

    Ok(Preset {
        name: fallback_name.unwrap_or_else(|| "Imported EQ".to_string()),
        preamp_db,
        filters,
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
                "peak" => FilterKind::Peak,
                other => bail!("unsupported filter kind {other}"),
            };

            filters.push(Filter {
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
        original_text: input.to_string(),
    })
}

fn extract_token<'a>(line: &'a str, start: &str, end: &str) -> Result<&'a str> {
    let after = line
        .split(start)
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("missing {start} in line: {line}"))?;
    let value = after
        .split(end)
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing {end} in line: {line}"))?;
    Ok(value.trim())
}
