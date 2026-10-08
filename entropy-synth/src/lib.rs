//! Layered synthesizer used to author Entropy Turbine's WAV files.
//!
//! Build a sound with [`Sound`], [`Oscillator`], [`Noise`], and [`Envelope`],
//! play it from the `entropy-synth` binary, then export the WAVs whose
//! definitions no longer match `assets/audio/sounds.ron`.

mod catalog;
mod manifest;
mod synth;
mod wav;

pub use catalog::{ExportItem, example_steam_shot, exported, preview};
pub use manifest::{ExportAction, ExportChange, export_all};
pub use synth::{Envelope, Noise, Oscillator, Sound, peak};

pub const SAMPLE_RATE: u32 = 44_100;
