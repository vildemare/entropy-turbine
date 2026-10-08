use std::f32::consts::TAU;

use sha2::{Digest, Sha256};

const ENGINE: &str = "1";

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Triangle,
    Square,
}

#[derive(Clone)]
enum Source {
    Oscillator {
        wave: Wave,
        frequency: f32,
        sweep_to: Option<f32>,
    },
    Noise {
        seed: u32,
    },
}

/// One oscillator or noise bed inside a [`Sound`].
///
/// A layer with no [`duration`](Layer::duration) plays from its start until the
/// sound ends. A layer with no envelope stays at full level for that window.
#[derive(Clone)]
pub struct Layer {
    source: Source,
    start_at: f32,
    duration: Option<f32>,
    gain: f32,
    envelope: Option<Envelope>,
    low_pass: Option<f32>,
    high_pass: Option<f32>,
}

impl Layer {
    fn oscillator(wave: Wave, frequency: f32) -> Self {
        Self {
            source: Source::Oscillator {
                wave,
                frequency,
                sweep_to: None,
            },
            start_at: 0.0,
            duration: None,
            gain: 1.0,
            envelope: None,
            low_pass: None,
            high_pass: None,
        }
    }

    pub fn sweep_to(mut self, frequency: f32) -> Self {
        if let Source::Oscillator { sweep_to, .. } = &mut self.source {
            *sweep_to = Some(frequency);
        }
        self
    }

    pub fn duration(mut self, seconds: f32) -> Self {
        self.duration = Some(seconds);
        self
    }

    pub fn start_at(mut self, seconds: f32) -> Self {
        self.start_at = seconds;
        self
    }

    pub fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    pub fn envelope(mut self, envelope: Envelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    pub fn low_pass(mut self, frequency: f32) -> Self {
        self.low_pass = Some(frequency);
        self
    }

    pub fn high_pass(mut self, frequency: f32) -> Self {
        self.high_pass = Some(frequency);
        self
    }

    /// Changes the noise sequence. Oscillators ignore it.
    pub fn seed(mut self, seed: u32) -> Self {
        if let Source::Noise { seed: stored } = &mut self.source {
            *stored = seed;
        }
        self
    }

    fn canonical(&self) -> String {
        let source = match &self.source {
            Source::Oscillator {
                wave,
                frequency,
                sweep_to,
            } => {
                let wave = match wave {
                    Wave::Sine => "sine",
                    Wave::Triangle => "triangle",
                    Wave::Square => "square",
                };
                format!(
                    "osc {wave} freq={} sweep={}",
                    num(*frequency),
                    sweep_to.map(num).unwrap_or_else(|| "none".to_string())
                )
            }
            Source::Noise { seed } => format!("noise white seed={seed}"),
        };
        let duration = self.duration.map(num).unwrap_or_else(|| "full".to_string());
        let envelope = self
            .envelope
            .map(|envelope| envelope.canonical())
            .unwrap_or_else(|| "none".to_string());
        format!(
            "{source} start={} dur={duration} gain={} env={envelope} lp={} hp={}",
            num(self.start_at),
            num(self.gain),
            self.low_pass.map(num).unwrap_or_else(|| "none".to_string()),
            self.high_pass
                .map(num)
                .unwrap_or_else(|| "none".to_string()),
        )
    }
}

pub struct Oscillator;

impl Oscillator {
    pub fn sine(frequency: f32) -> Layer {
        Layer::oscillator(Wave::Sine, frequency)
    }

    pub fn triangle(frequency: f32) -> Layer {
        Layer::oscillator(Wave::Triangle, frequency)
    }

    pub fn square(frequency: f32) -> Layer {
        Layer::oscillator(Wave::Square, frequency)
    }
}

pub struct Noise;

impl Noise {
    pub fn white() -> Layer {
        Layer {
            source: Source::Noise { seed: 1 },
            start_at: 0.0,
            duration: None,
            gain: 1.0,
            envelope: None,
            low_pass: None,
            high_pass: None,
        }
    }
}

/// Attack, decay, sustain level, and release, in seconds except sustain.
///
/// Sustain is a level from 0 to 1. Release occupies the end of the layer and
/// ramps from whatever level the envelope had reached down to silence.
#[derive(Clone, Copy)]
pub struct Envelope {
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            attack: 0.0,
            decay: 0.0,
            sustain: 1.0,
            release: 0.0,
        }
    }

    pub fn attack(mut self, seconds: f32) -> Self {
        self.attack = seconds;
        self
    }

    pub fn decay(mut self, seconds: f32) -> Self {
        self.decay = seconds;
        self
    }

    pub fn sustain(mut self, level: f32) -> Self {
        self.sustain = level;
        self
    }

    pub fn release(mut self, seconds: f32) -> Self {
        self.release = seconds;
        self
    }

    /// A short click that falls to silence.
    pub fn fast_decay() -> Self {
        Self {
            attack: 0.002,
            decay: 0.06,
            sustain: 0.0,
            release: 0.0,
        }
    }

    fn level(self, time: f32, duration: f32) -> f32 {
        let release_at = (duration - self.release.max(0.0)).max(0.0);
        if self.release > 0.0 && time >= release_at {
            let start = self.shaped(release_at);
            let along = ((time - release_at) / self.release).clamp(0.0, 1.0);
            return start * (1.0 - along);
        }
        self.shaped(time)
    }

    fn shaped(self, time: f32) -> f32 {
        if time < self.attack {
            if self.attack <= 1.0e-6 {
                return 1.0;
            }
            return (time / self.attack).clamp(0.0, 1.0);
        }
        let decay_end = self.attack + self.decay;
        if time < decay_end {
            if self.decay <= 1.0e-6 {
                return self.sustain;
            }
            let along = ((time - self.attack) / self.decay).clamp(0.0, 1.0);
            return 1.0 + (self.sustain - 1.0) * along;
        }
        self.sustain
    }

    fn canonical(self) -> String {
        format!(
            "{},{},{},{}",
            num(self.attack),
            num(self.decay),
            num(self.sustain),
            num(self.release)
        )
    }
}

impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}

/// A mixed sound of fixed length.
#[derive(Clone)]
pub struct Sound {
    duration: f32,
    layers: Vec<Layer>,
}

impl Sound {
    pub fn duration_secs(seconds: f32) -> Self {
        Self {
            duration: seconds.max(0.0),
            layers: Vec::new(),
        }
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Stable identity of this definition at `sample_rate`.
    ///
    /// The export ledger stores this. A changed layer, duration, or sample rate
    /// produces a new hash, and the WAV is rendered again.
    pub fn definition_hash(&self, sample_rate: u32) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical(sample_rate).as_bytes());
        hex(&hasher.finalize())
    }

    pub fn render(&self, sample_rate: u32) -> Vec<f32> {
        let count = sample_count(self.duration, sample_rate);
        let mut mix = vec![0.0; count];
        for layer in &self.layers {
            render_layer(layer, sample_rate, self.duration, &mut mix);
        }
        mix
    }

    fn canonical(&self, sample_rate: u32) -> String {
        let mut lines = vec![format!(
            "engine={ENGINE} rate={sample_rate} duration={}",
            num(self.duration)
        )];
        for layer in &self.layers {
            lines.push(layer.canonical());
        }
        lines.join("\n")
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max)
}

fn render_layer(layer: &Layer, sample_rate: u32, sound_duration: f32, mix: &mut [f32]) {
    let start = layer.start_at.max(0.0);
    let end = match layer.duration {
        Some(duration) => start + duration.max(0.0),
        None => sound_duration,
    }
    .min(sound_duration);
    if end <= start || sample_rate == 0 {
        return;
    }
    let first = sample_count(start, sample_rate).min(mix.len());
    let last = sample_count(end, sample_rate).min(mix.len());
    if last <= first {
        return;
    }
    let window = end - start;
    let rate = sample_rate as f32;
    let mut phase = 0.0f64;
    let mut noise = NoiseGenerator::new(match &layer.source {
        Source::Noise { seed } => *seed,
        Source::Oscillator { .. } => 1,
    });
    let mut low_state = 0.0f32;
    let mut high_prev_x = 0.0f32;
    let mut high_prev_y = 0.0f32;
    for index in first..last {
        let time = index as f32 / rate - start;
        let raw = match &layer.source {
            Source::Oscillator {
                wave,
                frequency,
                sweep_to,
            } => {
                let frequency = frequency_at(*frequency, *sweep_to, time, window);
                let sample = wave_sample(*wave, phase);
                phase += f64::from(TAU) * f64::from(frequency) / f64::from(rate);
                sample
            }
            Source::Noise { .. } => noise.next(),
        };
        let filtered = apply_filters(
            raw,
            rate,
            layer.high_pass,
            layer.low_pass,
            &mut high_prev_x,
            &mut high_prev_y,
            &mut low_state,
        );
        let level = layer
            .envelope
            .map(|envelope| envelope.level(time, window))
            .unwrap_or(1.0);
        mix[index] += filtered * level * layer.gain;
    }
}

fn wave_sample(wave: Wave, phase: f64) -> f32 {
    let wrapped = phase.rem_euclid(f64::from(TAU));
    match wave {
        Wave::Sine => wrapped.sin() as f32,
        Wave::Triangle => {
            let position = wrapped / f64::from(TAU);
            if position < 0.5 {
                (4.0 * position - 1.0) as f32
            } else {
                (3.0 - 4.0 * position) as f32
            }
        }
        Wave::Square => {
            if wrapped < f64::from(std::f32::consts::PI) {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn frequency_at(start: f32, end: Option<f32>, time: f32, duration: f32) -> f32 {
    let Some(end) = end else {
        return start.max(0.0);
    };
    if duration <= 1.0e-6 {
        return start.max(0.0);
    }
    let along = (time / duration).clamp(0.0, 1.0);
    if start <= 0.0 || end <= 0.0 {
        return (start + (end - start) * along).max(0.0);
    }
    start * (end / start).powf(along)
}

fn apply_filters(
    sample: f32,
    sample_rate: f32,
    high_pass: Option<f32>,
    low_pass: Option<f32>,
    high_prev_x: &mut f32,
    high_prev_y: &mut f32,
    low_state: &mut f32,
) -> f32 {
    let mut sample = sample;
    if let Some(cutoff) = high_pass {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (TAU * cutoff.max(1.0));
        let alpha = rc / (rc + dt);
        let output = alpha * (*high_prev_y + sample - *high_prev_x);
        *high_prev_x = sample;
        *high_prev_y = output;
        sample = output;
    }
    if let Some(cutoff) = low_pass {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (TAU * cutoff.max(1.0));
        let alpha = dt / (rc + dt);
        *low_state += alpha * (sample - *low_state);
        sample = *low_state;
    }
    sample
}

struct NoiseGenerator {
    state: u32,
}

impl NoiseGenerator {
    fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    fn next(&mut self) -> f32 {
        let mut state = self.state;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        if state == 0 {
            state = 1;
        }
        self.state = state;
        (state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn sample_count(duration: f32, sample_rate: u32) -> usize {
    (duration * sample_rate as f32).round().max(0.0) as usize
}

fn num(value: f32) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        format!("{value:.6}")
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_without_duration_fills_the_sound() {
        let sound = Sound::duration_secs(0.5).layer(Oscillator::sine(40.0).gain(0.5));
        let samples = sound.render(1_000);
        assert_eq!(samples.len(), 500);
        let late: f32 = samples[450..]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f32>()
            / 50.0;
        assert!(late.sqrt() > 0.2, "late rms {late}");
    }

    #[test]
    fn a_delayed_layer_is_silent_until_it_starts() {
        let sound = Sound::duration_secs(1.0).layer(
            Oscillator::sine(440.0)
                .start_at(0.5)
                .duration(0.5)
                .gain(0.8),
        );
        let samples = sound.render(44_100);
        assert!(samples[..1_000].iter().all(|sample| *sample == 0.0));
        assert!(samples[30_000].abs() > 0.2);
    }

    #[test]
    fn definition_hash_tracks_the_layers() {
        let quiet = Sound::duration_secs(0.2).layer(Oscillator::sine(100.0).gain(0.2));
        let louder = Sound::duration_secs(0.2).layer(Oscillator::sine(100.0).gain(0.3));
        assert_eq!(quiet.definition_hash(44_100), quiet.definition_hash(44_100));
        assert_ne!(
            quiet.definition_hash(44_100),
            louder.definition_hash(44_100)
        );
        assert_ne!(quiet.definition_hash(44_100), quiet.definition_hash(22_050));
    }

    #[test]
    fn noise_is_deterministic() {
        let sound = Sound::duration_secs(0.1).layer(Noise::white().seed(7).gain(0.5));
        assert_eq!(sound.render(8_000), sound.render(8_000));
    }
}
