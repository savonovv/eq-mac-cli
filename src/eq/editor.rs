use crate::eq::model::{Filter, FilterKind, Preset};
use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct EditorState {
    pub preset_name: String,
    pub preamp_db: f32,
    pub filters: Vec<Filter>,
    pub selected_index: usize,
    pub dirty: bool,
}

impl EditorState {
    pub fn new(preset_name: String) -> Self {
        Self {
            preset_name,
            preamp_db: 0.0,
            filters: vec![default_filter()],
            selected_index: 0,
            dirty: true,
        }
    }

    pub fn from_preset(preset: &Preset) -> Result<Self> {
        if !preset.channel_filters.is_empty() {
            bail!("channel-based presets are not editable in the TUI yet");
        }

        let filters = if preset.filters.is_empty() {
            vec![default_filter()]
        } else {
            preset.filters.clone()
        };

        Ok(Self {
            preset_name: preset.name.clone(),
            preamp_db: preset.preamp_db,
            filters,
            selected_index: 0,
            dirty: false,
        })
    }

    pub fn selected_filter(&self) -> &Filter {
        &self.filters[self.selected_index]
    }

    pub fn selected_filter_mut(&mut self) -> &mut Filter {
        &mut self.filters[self.selected_index]
    }

    pub fn next_filter(&mut self) {
        if !self.filters.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filters.len();
        }
    }

    pub fn previous_filter(&mut self) {
        if self.filters.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index == 0 {
            self.selected_index = self.filters.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    pub fn add_filter(&mut self) {
        self.filters.push(default_filter());
        self.selected_index = self.filters.len().saturating_sub(1);
        self.dirty = true;
    }

    pub fn delete_selected_filter(&mut self) {
        if self.filters.len() <= 1 {
            self.filters[0] = default_filter();
            self.selected_index = 0;
        } else {
            self.filters.remove(self.selected_index);
            if self.selected_index >= self.filters.len() {
                self.selected_index = self.filters.len() - 1;
            }
        }
        self.dirty = true;
    }

    pub fn toggle_selected_filter(&mut self) {
        let filter = self.selected_filter_mut();
        filter.enabled = !filter.enabled;
        self.dirty = true;
    }

    pub fn cycle_mode(&mut self) {
        let filter = self.selected_filter_mut();
        filter.kind = filter.kind.next();
        if !filter.kind.uses_gain() {
            filter.gain_db = 0.0;
        }
        self.dirty = true;
    }

    pub fn adjust_frequency(&mut self, delta_hz: f32) {
        let filter = self.selected_filter_mut();
        filter.frequency_hz = (filter.frequency_hz + delta_hz).clamp(10.0, 22_000.0);
        self.dirty = true;
    }

    pub fn adjust_gain(&mut self, delta: f32) {
        let filter = self.selected_filter_mut();
        if filter.kind.uses_gain() {
            filter.gain_db = (filter.gain_db + delta).clamp(-30.0, 30.0);
            self.dirty = true;
        }
    }

    pub fn reset_gain(&mut self) {
        let filter = self.selected_filter_mut();
        if filter.kind.uses_gain() {
            filter.gain_db = 0.0;
            self.dirty = true;
        }
    }

    pub fn adjust_q(&mut self, delta: f32) {
        let filter = self.selected_filter_mut();
        filter.q = (filter.q + delta).clamp(0.05, 50.0);
        self.dirty = true;
    }

    pub fn to_preset_text(&self) -> String {
        let mut lines = vec![format!("Preamp: {} dB", format_number(self.preamp_db))];
        for (index, filter) in self.filters.iter().enumerate() {
            let status = if filter.enabled { "ON" } else { "OFF" };
            let mut line = format!(
                "Filter {}: {} {} Fc {} Hz",
                index + 1,
                status,
                filter.kind.token(),
                format_number(filter.frequency_hz)
            );
            if filter.kind.uses_gain() {
                line.push_str(&format!(" Gain {} dB", format_number(filter.gain_db)));
            }
            line.push_str(&format!(" Q {}", format_number(filter.q)));
            lines.push(line);
        }
        lines.join("\n")
    }
}

fn default_filter() -> Filter {
    Filter {
        enabled: true,
        kind: FilterKind::Peak,
        frequency_hz: 1_000.0,
        gain_db: 0.0,
        q: 1.0,
    }
}

fn format_number(value: f32) -> String {
    if value.fract().abs() < 0.0001 {
        format!("{:.0}", value)
    } else {
        format!("{:.2}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::EditorState;
    use crate::eq::model::{Filter, FilterKind, Preset};

    #[test]
    fn serializes_filters_back_to_apo_style_text() {
        let state = EditorState {
            preset_name: "Test".to_string(),
            preamp_db: -2.5,
            filters: vec![Filter {
                enabled: true,
                kind: FilterKind::Peak,
                frequency_hz: 1000.0,
                gain_db: -3.0,
                q: 1.41,
            }],
            selected_index: 0,
            dirty: false,
        };

        let text = state.to_preset_text();
        assert!(text.contains("Preamp: -2.50 dB"));
        assert!(text.contains("Filter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41"));
    }

    #[test]
    fn rejects_channel_based_presets_for_now() {
        let preset = Preset {
            name: "x".to_string(),
            preamp_db: 0.0,
            filters: Vec::new(),
            channel_filters: vec![crate::eq::model::ChannelFilters {
                channel_name: "L".to_string(),
                filters: Vec::new(),
            }],
            original_text: String::new(),
        };

        assert!(EditorState::from_preset(&preset).is_err());
    }
}
