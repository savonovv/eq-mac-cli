use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub preamp_db: f32,
    pub filters: Vec<Filter>,
    pub original_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub kind: FilterKind,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FilterKind {
    LowShelf,
    Peak,
}
