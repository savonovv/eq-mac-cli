use crate::eq::model::{Filter, FilterKind, Preset};

#[derive(Debug, Clone)]
pub struct RuntimePreset {
    pub name: String,
    pub preamp_linear: f32,
    pub filters: Vec<FilterSpec>,
}

#[derive(Debug, Clone)]
pub struct FilterSpec {
    pub kind: FilterKind,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone)]
pub struct DspChain {
    gain: f32,
    filters: Vec<Vec<Biquad>>,
}

#[derive(Debug, Clone, Copy)]
struct BiquadCoefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Debug, Clone, Copy)]
struct Biquad {
    coeffs: BiquadCoefficients,
    z1: f32,
    z2: f32,
}

impl RuntimePreset {
    pub fn from_preset(preset: &Preset) -> Self {
        Self {
            name: preset.name.clone(),
            preamp_linear: db_to_linear(preset.preamp_db),
            filters: preset
                .filters
                .iter()
                .map(|filter| FilterSpec::from_filter(filter))
                .collect(),
        }
    }
}

impl FilterSpec {
    fn from_filter(filter: &Filter) -> Self {
        Self {
            kind: filter.kind,
            frequency_hz: filter.frequency_hz,
            gain_db: filter.gain_db,
            q: filter.q,
        }
    }
}

impl DspChain {
    pub fn bypass(channels: usize) -> Self {
        Self {
            gain: 1.0,
            filters: vec![Vec::new(); channels],
        }
    }

    pub fn from_runtime_preset(preset: &RuntimePreset, sample_rate: u32, channels: usize) -> Self {
        let mut per_channel = vec![Vec::new(); channels];
        for specs in &mut per_channel {
            for filter in &preset.filters {
                specs.push(Biquad::new(filter.coefficients(sample_rate)));
            }
        }

        Self {
            gain: preset.preamp_linear,
            filters: per_channel,
        }
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) {
        for (channel, sample) in frame.iter_mut().enumerate() {
            let mut current = *sample * self.gain;
            if let Some(filters) = self.filters.get_mut(channel) {
                for filter in filters {
                    current = filter.process(current);
                }
            }
            *sample = current;
        }
    }
}

impl FilterSpec {
    fn coefficients(&self, sample_rate: u32) -> BiquadCoefficients {
        match self.kind {
            FilterKind::Peak => {
                peaking_coefficients(sample_rate, self.frequency_hz, self.gain_db, self.q)
            }
            FilterKind::LowShelf => {
                low_shelf_coefficients(sample_rate, self.frequency_hz, self.gain_db, self.q)
            }
        }
    }
}

impl Biquad {
    fn new(coeffs: BiquadCoefficients) -> Self {
        Self {
            coeffs,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = input * self.coeffs.b0 + self.z1;
        self.z1 = input * self.coeffs.b1 + self.z2 - self.coeffs.a1 * output;
        self.z2 = input * self.coeffs.b2 - self.coeffs.a2 * output;
        output
    }
}

fn peaking_coefficients(
    sample_rate: u32,
    frequency_hz: f32,
    gain_db: f32,
    q: f32,
) -> BiquadCoefficients {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = normalized_frequency(sample_rate, frequency_hz);
    let alpha = w0.sin() / (2.0 * q.max(0.001));
    let cos_w0 = w0.cos();

    normalize(
        1.0 + alpha * a,
        -2.0 * cos_w0,
        1.0 - alpha * a,
        1.0 + alpha / a,
        -2.0 * cos_w0,
        1.0 - alpha / a,
    )
}

fn low_shelf_coefficients(
    sample_rate: u32,
    frequency_hz: f32,
    gain_db: f32,
    q: f32,
) -> BiquadCoefficients {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = normalized_frequency(sample_rate, frequency_hz);
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let slope = q.clamp(0.1, 4.0);
    let alpha = sin_w0 / 2.0 * (((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).max(0.0)).sqrt();
    let beta = 2.0 * a.sqrt() * alpha;

    normalize(
        a * ((a + 1.0) - (a - 1.0) * cos_w0 + beta),
        2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
        a * ((a + 1.0) - (a - 1.0) * cos_w0 - beta),
        (a + 1.0) + (a - 1.0) * cos_w0 + beta,
        -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
        (a + 1.0) + (a - 1.0) * cos_w0 - beta,
    )
}

fn normalized_frequency(sample_rate: u32, frequency_hz: f32) -> f32 {
    let nyquist = sample_rate as f32 / 2.0;
    let clamped = frequency_hz.clamp(10.0, nyquist - 10.0);
    2.0 * std::f32::consts::PI * clamped / sample_rate as f32
}

fn normalize(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> BiquadCoefficients {
    BiquadCoefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
