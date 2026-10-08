use std::env;
use std::num::NonZero;
use std::path::PathBuf;
use std::process::ExitCode;

use entropy_synth::{ExportAction, SAMPLE_RATE, export_all, exported, peak, preview};

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let game = take_game_dir(&mut args);
    match args.first().map(String::as_str) {
        Some("list") => {
            println!("exported:");
            for item in exported() {
                println!("  {}", item.id);
            }
            println!("preview:");
            println!("  example");
            ExitCode::SUCCESS
        }
        Some("play") => {
            let Some(name) = args.get(1) else {
                eprintln!("usage: entropy-synth play <sound> [repeats]");
                return ExitCode::from(2);
            };
            let repeats = args
                .get(2)
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|repeats| *repeats > 0)
                .unwrap_or(1);
            let Some(sound) = preview(name) else {
                eprintln!("unknown sound {name}");
                eprintln!("run `entropy-synth list` for the names");
                return ExitCode::from(2);
            };
            let mut samples = sound.render(SAMPLE_RATE);
            let level = peak(&samples);
            println!("{name}  {:.3} s  peak {level:.2}", sound.duration());
            if repeats > 1 {
                let once = samples.clone();
                let gap = (0.12 * SAMPLE_RATE as f32) as usize;
                for _ in 1..repeats {
                    samples.extend(std::iter::repeat(0.0).take(gap));
                    samples.extend_from_slice(&once);
                }
            }
            if let Err(error) = play(samples) {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Some("export") => {
            let force = args.iter().any(|arg| arg == "--force");
            let audio = game.join("assets/audio");
            match export_all(&audio, &exported(), force) {
                Ok(changes) => {
                    for change in changes {
                        let action = match change.action {
                            ExportAction::Wrote => {
                                let level = change.peak.unwrap_or(0.0);
                                format!("wrote  peak {level:.2}")
                            }
                            ExportAction::Kept => "unchanged".to_string(),
                            ExportAction::Recorded => {
                                let path = audio.join(&change.file);
                                if path.is_file() {
                                    "recorded, left alone".to_string()
                                } else {
                                    "recorded, file missing".to_string()
                                }
                            }
                        };
                        println!("{:<16} {action}", change.id);
                    }
                    println!("ledger {}", audio.join("sounds.ron").display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!(
                "\
usage:
  entropy-synth play <sound> [repeats]
  entropy-synth export [--force]
  entropy-synth list

Sounds are rendered in memory for play. export writes a WAV only when
assets/audio/sounds.ron has a different definition hash, then updates that
ledger. Recorded entries in the ledger are not replaced.

  --game <path>   game directory (default: this repo)
"
            );
            ExitCode::from(2)
        }
    }
}

fn take_game_dir(args: &mut Vec<String>) -> PathBuf {
    if let Some(index) = args.iter().position(|arg| arg == "--game") {
        let path = if index + 1 < args.len() {
            args.remove(index + 1)
        } else {
            String::new()
        };
        args.remove(index);
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Ok(path) = env::var("ENTROPY_GAME") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn play(samples: Vec<f32>) -> Result<(), String> {
    let mut handle = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|error| format!("no audio output: {error}"))?;
    handle.log_on_drop(false);
    let player = rodio::Player::connect_new(handle.mixer());
    player.append(rodio::buffer::SamplesBuffer::new(
        NonZero::new(1).expect("one channel"),
        NonZero::new(SAMPLE_RATE).expect("sample rate"),
        samples,
    ));
    player.sleep_until_end();
    Ok(())
}
