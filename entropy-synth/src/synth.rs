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

/// Where a layer starts, before the sound has been scheduled.
#[derive(Clone)]
enum Timing {
    At(f32),
    AfterStart(String, f32),
    BeforeStart(String, f32),
    AfterEnd(String, f32),
    BeforeEnd(String, f32),
    /// The end of the previous layer. The first layer starts at 0.
    AfterPrevious,
}

/// One oscillator or noise bed inside a [`Sound`].
///
/// A layer with no [`duration`](Layer::duration) plays from its start until the
/// sound's content ends. A layer with no envelope stays at full level for that
/// window. [`after_end`](Layer::after_end) and the other anchors place this
/// layer against a named layer. The anchor reads that layer's own duration, so
/// the named layer needs [`duration`](Layer::duration).
#[derive(Clone)]
pub struct Layer {
    name: Option<String>,
    timing: Timing,
    source: Source,
    duration: Option<f32>,
    gain: f32,
    envelope: Option<Envelope>,
    low_pass: Option<f32>,
    high_pass: Option<f32>,
}

impl Layer {
    fn oscillator(wave: Wave, frequency: f32) -> Self {
        Self {
            name: None,
            timing: Timing::At(0.0),
            source: Source::Oscillator {
                wave,
                frequency,
                sweep_to: None,
            },
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
        self.timing = Timing::At(seconds);
        self
    }

    /// Start this many seconds after the named layer starts.
    pub fn after_start(mut self, layer: &str, seconds: f32) -> Self {
        self.timing = Timing::AfterStart(layer.to_string(), seconds);
        self
    }

    /// Start this many seconds before the named layer starts.
    pub fn before_start(mut self, layer: &str, seconds: f32) -> Self {
        self.timing = Timing::BeforeStart(layer.to_string(), seconds);
        self
    }

    /// Start this many seconds after the named layer ends.
    pub fn after_end(mut self, layer: &str, seconds: f32) -> Self {
        self.timing = Timing::AfterEnd(layer.to_string(), seconds);
        self
    }

    /// Start this many seconds before the named layer ends.
    pub fn before_end(mut self, layer: &str, seconds: f32) -> Self {
        self.timing = Timing::BeforeEnd(layer.to_string(), seconds);
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

    fn canonical(&self, start: f32) -> String {
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
            num(start),
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
            name: None,
            timing: Timing::At(0.0),
            source: Source::Noise { seed: 1 },
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

/// A mixed sound.
///
/// With no [`fixed_duration`](Sound::fixed_duration), the length is the latest
/// layer end. [`padding`](Sound::padding) adds silence after that.
/// [`duration_secs`](Sound::duration_secs) fixes the length from the start.
#[derive(Clone)]
pub struct Sound {
    fixed_duration: Option<f32>,
    padding: f32,
    layers: Vec<Layer>,
}

struct Schedule {
    /// Layer audio ends here. Padding stays silent after it.
    content: f32,
    total: f32,
    layers: Vec<(f32, Layer)>,
}

impl Sound {
    pub fn new() -> Self {
        Self {
            fixed_duration: None,
            padding: 0.0,
            layers: Vec::new(),
        }
    }

    pub fn duration_secs(seconds: f32) -> Self {
        Self::new().fixed_duration(seconds)
    }

    /// Keep the sound at this length, then add any [`padding`](Sound::padding).
    ///
    /// Layers are clipped to this length. A sound with no fixed duration runs
    /// until the latest layer end instead.
    pub fn fixed_duration(mut self, seconds: f32) -> Self {
        self.fixed_duration = Some(seconds.max(0.0));
        self
    }

    /// Silence added after the sound's content.
    pub fn padding(mut self, seconds: f32) -> Self {
        self.padding = seconds.max(0.0);
        self
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }

    /// Add a layer under `name`, so other layers can anchor to it.
    pub fn layer_named(mut self, name: &str, mut layer: Layer) -> Self {
        if name.is_empty() {
            panic!("a layer name is empty");
        }
        if self
            .layers
            .iter()
            .any(|existing| existing.name.as_deref() == Some(name))
        {
            panic!("sound already has a layer named {name}");
        }
        layer.name = Some(name.to_string());
        self.layers.push(layer);
        self
    }

    /// Add a layer that starts when the previous layer ends.
    ///
    /// The previous layer needs its own duration. The first layer starts at 0.
    pub fn append_layer(mut self, mut layer: Layer) -> Self {
        layer.timing = Timing::AfterPrevious;
        self.layers.push(layer);
        self
    }

    pub fn duration(&self) -> f32 {
        self.schedule().total
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
        let schedule = self.schedule();
        let count = sample_count(schedule.total, sample_rate);
        let mut mix = vec![0.0; count];
        for (start, layer) in &schedule.layers {
            render_layer(layer, *start, sample_rate, schedule.content, &mut mix);
        }
        mix
    }

    fn canonical(&self, sample_rate: u32) -> String {
        let schedule = self.schedule();
        let mut header = format!(
            "engine={ENGINE} rate={sample_rate} duration={}",
            num(schedule.total)
        );
        if self.padding > 0.0 {
            header.push_str(&format!(" pad={}", num(self.padding)));
        }
        let mut lines = vec![header];
        for (start, layer) in &schedule.layers {
            lines.push(layer.canonical(*start));
        }
        lines.join("\n")
    }

    fn schedule(&self) -> Schedule {
        let count = self.layers.len();
        let mut starts = vec![None; count];
        let mut ends = vec![None; count];
        for _ in 0..count {
            let mut advanced = false;
            for index in 0..count {
                if starts[index].is_some() {
                    continue;
                }
                let Some(start) = self.anchor_start(index, &starts, &ends) else {
                    continue;
                };
                starts[index] = Some(start);
                if let Some(duration) = self.layers[index].duration {
                    ends[index] = Some(start + duration.max(0.0));
                }
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
        if starts.iter().any(Option::is_none) {
            let waiting = (0..count)
                .filter(|&index| starts[index].is_none())
                .map(|index| self.anchor_label(index))
                .collect::<Vec<_>>()
                .join(", ");
            panic!("layer timing is still waiting: {waiting}");
        }

        if self.fixed_duration.is_none() {
            // Time zero stays put. A start before zero pulls every layer forward
            // so that layer is still heard.
            let earliest = starts.iter().flatten().copied().fold(0.0, f32::min);
            if earliest < 0.0 {
                let shift = -earliest;
                for start in &mut starts {
                    *start = Some(start.unwrap() + shift);
                }
                for end in &mut ends {
                    if let Some(end) = end {
                        *end += shift;
                    }
                }
            }
        } else {
            for index in 0..count {
                let start = starts[index].unwrap().max(0.0);
                starts[index] = Some(start);
                if let Some(duration) = self.layers[index].duration {
                    ends[index] = Some(start + duration.max(0.0));
                }
            }
        }

        let content = self
            .fixed_duration
            .unwrap_or_else(|| ends.iter().flatten().copied().fold(0.0_f32, f32::max));
        let layers = self
            .layers
            .iter()
            .zip(starts)
            .map(|(layer, start)| (start.unwrap(), layer.clone()))
            .collect();
        Schedule {
            content,
            total: content + self.padding.max(0.0),
            layers,
        }
    }

    fn anchor_start(
        &self,
        index: usize,
        starts: &[Option<f32>],
        ends: &[Option<f32>],
    ) -> Option<f32> {
        match &self.layers[index].timing {
            Timing::At(seconds) => Some(*seconds),
            Timing::AfterStart(name, seconds) => {
                self.named_start(name, starts).map(|start| start + seconds)
            }
            Timing::BeforeStart(name, seconds) => {
                self.named_start(name, starts).map(|start| start - seconds)
            }
            Timing::AfterEnd(name, seconds) => self.named_end(name, ends).map(|end| end + seconds),
            Timing::BeforeEnd(name, seconds) => self.named_end(name, ends).map(|end| end - seconds),
            Timing::AfterPrevious => {
                if index == 0 {
                    return Some(0.0);
                }
                let previous = index - 1;
                if self.layers[previous].duration.is_none() {
                    panic!(
                        "{} has no duration, so the next layer cannot follow its end",
                        self.layer_label(previous)
                    );
                }
                ends[previous]
            }
        }
    }

    fn named_start(&self, name: &str, starts: &[Option<f32>]) -> Option<f32> {
        starts[self.layer_index(name)]
    }

    fn named_end(&self, name: &str, ends: &[Option<f32>]) -> Option<f32> {
        let index = self.layer_index(name);
        if self.layers[index].duration.is_none() {
            panic!(
                "{} has no duration, so another layer cannot anchor to its end",
                self.layer_label(index)
            );
        }
        ends[index]
    }

    fn layer_index(&self, name: &str) -> usize {
        self.layers
            .iter()
            .position(|layer| layer.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("sound has no layer named {name}"))
    }

    fn layer_label(&self, index: usize) -> String {
        match &self.layers[index].name {
            Some(name) => format!("layer {name}"),
            None => format!("layer {}", index + 1),
        }
    }

    fn anchor_label(&self, index: usize) -> String {
        let target = match &self.layers[index].timing {
            Timing::At(_) => "a fixed time".to_string(),
            Timing::AfterStart(name, _) | Timing::BeforeStart(name, _) => {
                format!("the start of {name}")
            }
            Timing::AfterEnd(name, _) | Timing::BeforeEnd(name, _) => format!("the end of {name}"),
            Timing::AfterPrevious => "the previous layer".to_string(),
        };
        format!("{} on {target}", self.layer_label(index))
    }
}

impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max)
}

fn render_layer(layer: &Layer, start: f32, sample_rate: u32, sound_duration: f32, mix: &mut [f32]) {
    let start = start.max(0.0);
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

    #[test]
    fn a_fixed_duration_keeps_the_same_definition_text() {
        let sound = Sound::duration_secs(0.2).layer(Oscillator::sine(100.0).gain(0.2));
        let same = Sound::new()
            .fixed_duration(0.2)
            .layer(Oscillator::sine(100.0).gain(0.2));
        assert_eq!(
            sound.canonical(44_100),
            "\
engine=1 rate=44100 duration=0.200000
osc sine freq=100.000000 sweep=none start=0 dur=full gain=0.200000 env=none lp=none hp=none"
        );
        assert_eq!(sound.definition_hash(44_100), same.definition_hash(44_100));
    }

    #[test]
    fn named_layers_follow_each_other_and_padding_is_silence() {
        let sound = Sound::new()
            .layer_named("hiss", Noise::white().duration(0.80).gain(0.0))
            .layer_named(
                "clunk",
                Oscillator::sine(110.0)
                    .after_end("hiss", 0.05)
                    .duration(0.15)
                    .gain(0.0),
            )
            .layer_named(
                "ring",
                Oscillator::triangle(720.0)
                    .before_end("clunk", 0.03)
                    .duration(0.40)
                    .gain(0.5),
            );
        let ring_end = 0.80 + 0.05 + 0.15 - 0.03 + 0.40;
        assert!((sound.duration() - ring_end).abs() < 1.0e-5);

        let padded = sound.clone().padding(0.20);
        assert!((padded.duration() - (ring_end + 0.20)).abs() < 1.0e-5);
        let samples = padded.render(1_000);
        assert_eq!(samples.len(), 1_570);
        assert!(samples[1_400..].iter().all(|sample| *sample == 0.0));
        assert!(
            samples[1_000..1_100]
                .iter()
                .any(|sample| sample.abs() > 0.1)
        );
    }

    #[test]
    fn an_appended_layer_starts_when_the_previous_one_ends() {
        let sound = Sound::new()
            .layer(Oscillator::sine(100.0).duration(0.20).gain(0.0))
            .append_layer(Oscillator::sine(440.0).duration(0.10).gain(0.8));
        assert!((sound.duration() - 0.30).abs() < 1.0e-5);
        let samples = sound.render(1_000);
        assert!(samples[..150].iter().all(|sample| *sample == 0.0));
        assert!(samples[200..280].iter().any(|sample| sample.abs() > 0.2));
    }

    #[test]
    fn a_layer_before_the_start_pulls_the_sound_forward() {
        let sound = Sound::new()
            .layer_named("hiss", Oscillator::sine(40.0).duration(0.20).gain(0.0))
            .layer_named(
                "lead",
                Oscillator::sine(440.0)
                    .before_start("hiss", 0.10)
                    .duration(0.10)
                    .gain(0.8),
            );
        assert!((sound.duration() - 0.30).abs() < 1.0e-5);
        let samples = sound.render(1_000);
        assert!(samples[..80].iter().any(|sample| sample.abs() > 0.2));
        assert!(samples[200].abs() < 0.01);
    }

    #[test]
    #[should_panic(expected = "sound has no layer named missing")]
    fn an_unknown_anchor_names_the_missing_layer() {
        let _ = Sound::new()
            .layer_named("hiss", Noise::white().duration(0.2))
            .layer(
                Oscillator::sine(110.0)
                    .after_end("missing", 0.0)
                    .duration(0.1),
            )
            .duration();
    }
}
