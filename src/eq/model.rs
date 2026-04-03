use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub preamp_db: f32,
    pub filters: Vec<Filter>,
    pub channel_filters: Vec<ChannelFilters>,
    pub original_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelFilters {
    pub channel_name: String,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub enabled: bool,
    pub kind: FilterKind,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FilterKind {
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
    Peak,
}

impl FilterKind {
    pub fn token(self) -> &'static str {
        match self {
            Self::LowShelf => "LS",
            Self::HighShelf => "HS",
            Self::HighPass => "HP",
            Self::LowPass => "LP",
            Self::Peak => "PK",
        }
    }

    pub fn uses_gain(self) -> bool {
        matches!(self, Self::LowShelf | Self::HighShelf | Self::Peak)
    }

    pub fn next(self) -> Self {
        match self {
            Self::LowShelf => Self::HighShelf,
            Self::HighShelf => Self::HighPass,
            Self::HighPass => Self::LowPass,
            Self::LowPass => Self::Peak,
            Self::Peak => Self::LowShelf,
        }
    }
}
