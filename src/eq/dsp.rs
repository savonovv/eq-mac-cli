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
                .filter(|filter| filter.enabled)
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
            FilterKind::HighShelf => {
                high_shelf_coefficients(sample_rate, self.frequency_hz, self.gain_db, self.q)
            }
            FilterKind::HighPass => high_pass_coefficients(sample_rate, self.frequency_hz, self.q),
            FilterKind::LowPass => low_pass_coefficients(sample_rate, self.frequency_hz, self.q),
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

fn high_shelf_coefficients(
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
        a * ((a + 1.0) + (a - 1.0) * cos_w0 + beta),
        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
        a * ((a + 1.0) + (a - 1.0) * cos_w0 - beta),
        (a + 1.0) - (a - 1.0) * cos_w0 + beta,
        2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
        (a + 1.0) - (a - 1.0) * cos_w0 - beta,
    )
}

fn high_pass_coefficients(sample_rate: u32, frequency_hz: f32, q: f32) -> BiquadCoefficients {
    let w0 = normalized_frequency(sample_rate, frequency_hz);
    let alpha = w0.sin() / (2.0 * q.max(0.001));
    let cos_w0 = w0.cos();

    normalize(
        (1.0 + cos_w0) / 2.0,
        -(1.0 + cos_w0),
        (1.0 + cos_w0) / 2.0,
        1.0 + alpha,
        -2.0 * cos_w0,
        1.0 - alpha,
    )
}

fn low_pass_coefficients(sample_rate: u32, frequency_hz: f32, q: f32) -> BiquadCoefficients {
    let w0 = normalized_frequency(sample_rate, frequency_hz);
    let alpha = w0.sin() / (2.0 * q.max(0.001));
    let cos_w0 = w0.cos();

    normalize(
        (1.0 - cos_w0) / 2.0,
        1.0 - cos_w0,
        (1.0 - cos_w0) / 2.0,
        1.0 + alpha,
        -2.0 * cos_w0,
        1.0 - alpha,
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

#[cfg(test)]
mod tests {
    use super::{
        high_pass_coefficients, high_shelf_coefficients, low_pass_coefficients,
        low_shelf_coefficients, peaking_coefficients, BiquadCoefficients,
    };

    fn magnitude_db(coeffs: BiquadCoefficients, sample_rate: u32, frequency_hz: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * frequency_hz / sample_rate as f32;
        let z1 = (omega.cos(), -omega.sin());
        let z2 = ((2.0 * omega).cos(), -(2.0 * omega).sin());

        let numerator = add_complex(
            add_complex((coeffs.b0, 0.0), scale_complex(z1, coeffs.b1)),
            scale_complex(z2, coeffs.b2),
        );
        let denominator = add_complex(
            add_complex((1.0, 0.0), scale_complex(z1, coeffs.a1)),
            scale_complex(z2, coeffs.a2),
        );
        let response = div_complex(numerator, denominator);
        20.0 * magnitude(response).log10()
    }

    fn add_complex(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
        (a.0 + b.0, a.1 + b.1)
    }

    fn scale_complex(value: (f32, f32), scalar: f32) -> (f32, f32) {
        (value.0 * scalar, value.1 * scalar)
    }

    fn div_complex(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
        let denom = b.0 * b.0 + b.1 * b.1;
        (
            (a.0 * b.0 + a.1 * b.1) / denom,
            (a.1 * b.0 - a.0 * b.1) / denom,
        )
    }

    fn magnitude(value: (f32, f32)) -> f32 {
        (value.0 * value.0 + value.1 * value.1).sqrt()
    }

    #[test]
    fn peak_filter_boosts_its_center_frequency() {
        let sample_rate = 48_000;
        let coeffs = peaking_coefficients(sample_rate, 1_000.0, 6.0, 1.0);

        let center = magnitude_db(coeffs, sample_rate, 1_000.0);
        let far = magnitude_db(coeffs, sample_rate, 10_000.0);

        assert!((center - 6.0).abs() < 0.2, "center={center}");
        assert!(far.abs() < 0.5, "far={far}");
    }

    #[test]
    fn low_shelf_hits_expected_dc_gain() {
        let sample_rate = 48_000;
        let coeffs = low_shelf_coefficients(sample_rate, 1_000.0, 6.0, 0.707);

        let low = magnitude_db(coeffs, sample_rate, 1.0);
        let high = magnitude_db(coeffs, sample_rate, 10_000.0);

        assert!((low - 6.0).abs() < 0.2, "low={low}");
        assert!(high.abs() < 0.5, "high={high}");
    }

    #[test]
    fn low_shelf_at_28_hz_is_subtle_above_60_hz() {
        let sample_rate = 48_000;
        let coeffs = low_shelf_coefficients(sample_rate, 28.0, 2.2, 0.917);

        let at_20 = magnitude_db(coeffs, sample_rate, 20.0);
        let at_60 = magnitude_db(coeffs, sample_rate, 60.0);
        let at_100 = magnitude_db(coeffs, sample_rate, 100.0);

        assert!(at_20 > 1.0, "at_20={at_20}");
        assert!(at_60 < 0.25, "at_60={at_60}");
        assert!(at_100 < 0.1, "at_100={at_100}");
    }

    #[test]
    fn high_shelf_hits_expected_high_frequency_gain() {
        let sample_rate = 48_000;
        let coeffs = high_shelf_coefficients(sample_rate, 4_000.0, 6.0, 0.707);

        let low = magnitude_db(coeffs, sample_rate, 100.0);
        let high = magnitude_db(coeffs, sample_rate, 20_000.0);

        assert!(low.abs() < 0.5, "low={low}");
        assert!((high - 6.0).abs() < 0.2, "high={high}");
    }

    #[test]
    fn high_pass_rejects_low_frequencies() {
        let sample_rate = 48_000;
        let coeffs = high_pass_coefficients(sample_rate, 200.0, 0.707);

        let low = magnitude_db(coeffs, sample_rate, 20.0);
        let high = magnitude_db(coeffs, sample_rate, 2_000.0);

        assert!(low < -15.0, "low={low}");
        assert!(high.abs() < 0.5, "high={high}");
    }

    #[test]
    fn low_pass_rejects_high_frequencies() {
        let sample_rate = 48_000;
        let coeffs = low_pass_coefficients(sample_rate, 2_000.0, 0.707);

        let low = magnitude_db(coeffs, sample_rate, 200.0);
        let high = magnitude_db(coeffs, sample_rate, 10_000.0);

        assert!(low.abs() < 0.5, "low={low}");
        assert!(high < -20.0, "high={high}");
    }
}
