use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::SAMPLE_RATE;
use crate::catalog::ExportItem;
use crate::synth::peak;
use crate::wav::write_wav;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportAction {
    Wrote,
    Kept,
    Recorded,
}

#[derive(Debug)]
pub struct ExportChange {
    pub id: String,
    pub file: String,
    pub action: ExportAction,
    pub peak: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct Manifest {
    sample_rate: u32,
    sounds: Vec<SoundRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct SoundRecord {
    id: String,
    file: String,
    origin: Origin,
    definition: String,
    duration_secs: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum Origin {
    #[serde(rename = "synthesized")]
    Synthesized,
    #[serde(rename = "recorded")]
    Recorded,
}

/// Render catalog sounds into `audio_dir` and refresh `sounds.ron`.
///
/// A synthesized file is written when it is missing, when `force` is set, or
/// when the stored definition hash differs. Recorded entries stay in the list
/// and their files are not replaced.
pub fn export_all(
    audio_dir: &Path,
    items: &[ExportItem],
    force: bool,
) -> Result<Vec<ExportChange>, String> {
    fs::create_dir_all(audio_dir).map_err(|error| error.to_string())?;
    let ledger_path = audio_dir.join("sounds.ron");
    let existing = load_manifest(&ledger_path)?;
    let mut records = Vec::new();
    let mut changes = Vec::new();

    for item in items {
        let definition = item.sound.definition_hash(SAMPLE_RATE);
        let previous = existing
            .as_ref()
            .and_then(|manifest| manifest.sounds.iter().find(|record| record.id == item.id));
        let path = audio_dir.join(&item.file);
        let hash_matches = previous.is_some_and(|record| {
            record.origin == Origin::Synthesized && record.definition == definition
        });
        let write = force || !hash_matches || !path.is_file();
        let level = if write {
            let samples = item.sound.render(SAMPLE_RATE);
            let level = peak(&samples);
            write_wav(&path, SAMPLE_RATE, &samples)
                .map_err(|error| format!("writing {}: {error}", path.display()))?;
            Some(level)
        } else {
            None
        };
        records.push(SoundRecord {
            id: item.id.to_string(),
            file: item.file.to_string(),
            origin: Origin::Synthesized,
            definition,
            duration_secs: item.sound.duration(),
        });
        changes.push(ExportChange {
            id: item.id.to_string(),
            file: item.file.to_string(),
            action: if write {
                ExportAction::Wrote
            } else {
                ExportAction::Kept
            },
            peak: level,
        });
    }

    if let Some(manifest) = existing {
        for record in manifest.sounds {
            if records.iter().any(|kept| kept.id == record.id) {
                continue;
            }
            if record.origin == Origin::Recorded {
                changes.push(ExportChange {
                    id: record.id.clone(),
                    file: record.file.clone(),
                    action: ExportAction::Recorded,
                    peak: None,
                });
            }
            records.push(record);
        }
    }

    save_manifest(
        &ledger_path,
        &Manifest {
            sample_rate: SAMPLE_RATE,
            sounds: records,
        },
    )?;
    Ok(changes)
}

fn load_manifest(path: &Path) -> Result<Option<Manifest>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    ron::from_str(&text).map(Some).map_err(|error| {
        format!(
            "{} could not be read, so nothing was exported: {error}",
            path.display()
        )
    })
}

fn save_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    let body = ron::ser::to_string_pretty(manifest, ron::ser::PrettyConfig::new())
        .map_err(|error| error.to_string())?;
    let text = format!(
        "// Sound ledger for Entropy Turbine.\n// synthesized: entropy-synth renders the WAV again when definition changes.\n// recorded: a WAV added by hand. The tool leaves the file alone.\n{body}\n"
    );
    let temporary = PathBuf::from(format!("{}.partial", path.display()));
    fs::write(&temporary, text).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{Oscillator, Sound};

    fn tick(gain: f32) -> ExportItem {
        ExportItem {
            id: "tick",
            file: "tick.wav".to_string(),
            sound: Sound::duration_secs(0.05).layer(Oscillator::sine(440.0).gain(gain)),
        }
    }

    #[test]
    fn export_rewrites_only_a_changed_definition_and_keeps_recordings() {
        let dir = std::env::temp_dir().join(format!("entropy-synth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let first = export_all(&dir, &[tick(0.2)], false).unwrap();
        assert_eq!(first[0].action, ExportAction::Wrote);
        assert!(dir.join("tick.wav").is_file());
        let written = fs::metadata(dir.join("tick.wav"))
            .unwrap()
            .modified()
            .unwrap();

        let second = export_all(&dir, &[tick(0.2)], false).unwrap();
        assert_eq!(second[0].action, ExportAction::Kept);
        let kept = fs::metadata(dir.join("tick.wav"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(written, kept);

        let ledger = dir.join("sounds.ron");
        let mut manifest: Manifest = ron::from_str(&fs::read_to_string(&ledger).unwrap()).unwrap();
        manifest.sounds.push(SoundRecord {
            id: "horn".to_string(),
            file: "horn.wav".to_string(),
            origin: Origin::Recorded,
            definition: String::new(),
            duration_secs: 1.0,
        });
        save_manifest(&ledger, &manifest).unwrap();
        fs::write(dir.join("horn.wav"), b"keep-me").unwrap();

        let third = export_all(&dir, &[tick(0.45)], false).unwrap();
        assert_eq!(third[0].action, ExportAction::Wrote);
        assert!(
            third
                .iter()
                .any(|change| change.id == "horn" && change.action == ExportAction::Recorded)
        );
        assert_eq!(fs::read(dir.join("horn.wav")).unwrap(), b"keep-me");
        let restored: Manifest = ron::from_str(&fs::read_to_string(&ledger).unwrap()).unwrap();
        assert!(restored.sounds.iter().any(|record| record.id == "horn"));
        assert_ne!(
            restored
                .sounds
                .iter()
                .find(|record| record.id == "tick")
                .unwrap()
                .definition,
            manifest
                .sounds
                .iter()
                .find(|record| record.id == "tick")
                .unwrap()
                .definition
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
