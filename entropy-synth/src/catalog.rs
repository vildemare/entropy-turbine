use crate::synth::{Envelope, Noise, Oscillator, Sound};

/// One synthesized file the game keeps in `assets/audio/`.
#[derive(Clone)]
pub struct ExportItem {
    pub id: &'static str,
    pub file: String,
    pub sound: Sound,
}

pub fn exported() -> Vec<ExportItem> {
    vec![
        item("player_shot", player_shot(1.0, 1)),
        item("player_shot_2", player_shot(0.86, 2)),
        item("player_shot_3", player_shot(1.08, 3)),
        item("enemy_shot", enemy_shot(1.0, 4)),
        item("enemy_shot_2", enemy_shot(0.84, 5)),
        item("enemy_shot_3", enemy_shot(1.12, 6)),
        item("impact", impact()),
        item("enemy_hit", enemy_hit()),
        item("player_hurt", player_hurt()),
        item("death", death()),
        item("menu_click", menu_click()),
        item("ambience", ambience()),
    ]
}

pub fn preview(name: &str) -> Option<Sound> {
    match name {
        "example" => Some(example_steam_shot()),
        other => exported()
            .into_iter()
            .find(|item| item.id == other)
            .map(|item| item.sound),
    }
}

/// The long plasma shot used as the shape of the builder API.
///
/// It is for listening in the tool. The exported player shots are the same
/// layers pressed shorter, so a shot every 140 ms does not pile into a wall.
pub fn example_steam_shot() -> Sound {
    Sound::duration_secs(1.5)
        .layer(
            Oscillator::sine(90.0).gain(0.35).envelope(
                Envelope::new()
                    .attack(0.005)
                    .decay(0.25)
                    .sustain(0.0)
                    .release(0.0),
            ),
        )
        .layer(
            Oscillator::sine(260.0)
                .sweep_to(75.0)
                .duration(0.40)
                .gain(0.45)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.40)
                        .sustain(0.0)
                        .release(0.0),
                ),
        )
        .layer(
            Oscillator::triangle(180.0)
                .start_at(0.025)
                .duration(0.28)
                .gain(0.22)
                .envelope(
                    Envelope::new()
                        .attack(0.01)
                        .decay(0.18)
                        .sustain(0.2)
                        .release(0.09),
                ),
        )
        .layer(Oscillator::square(1100.0).duration(0.018).gain(0.12))
        .layer(
            Oscillator::square(620.0)
                .start_at(0.46)
                .duration(0.025)
                .gain(0.09),
        )
        .layer(
            Noise::white()
                .start_at(0.008)
                .duration(0.22)
                .low_pass(4200.0)
                .gain(0.40)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.15)
                        .sustain(0.10)
                        .release(0.07),
                ),
        )
        .layer(
            Noise::white()
                .seed(11)
                .start_at(0.12)
                .duration(0.85)
                .high_pass(700.0)
                .low_pass(5500.0)
                .gain(0.16)
                .envelope(
                    Envelope::new()
                        .attack(0.08)
                        .decay(0.20)
                        .sustain(0.50)
                        .release(0.30),
                ),
        )
        .layer(
            Oscillator::triangle(740.0)
                .start_at(0.72)
                .duration(0.50)
                .sweep_to(690.0)
                .gain(0.08)
                .envelope(
                    Envelope::new()
                        .attack(0.002)
                        .decay(0.20)
                        .sustain(0.15)
                        .release(0.25),
                ),
        )
        .layer(
            Oscillator::sine(115.0)
                .start_at(1.18)
                .duration(0.16)
                .gain(0.30)
                .envelope(Envelope::fast_decay()),
        )
}

fn item(id: &'static str, sound: Sound) -> ExportItem {
    ExportItem {
        id,
        file: format!("{id}.wav"),
        sound,
    }
}

fn player_shot(pitch: f32, seed: u32) -> Sound {
    let hz = |frequency: f32| frequency * pitch;

    Sound::duration_secs(0.25)
        .layer(
            Oscillator::sine(hz(90.0))
                .gain(0.20)
                .envelope(
                    Envelope::new()
                        .attack(0.005)
                        .decay(0.12)
                        .sustain(0.0)
                        .release(0.0),
                )
        )
        .layer(
            Oscillator::sine(230.0)
                .sweep_to(22.0)
                .duration(0.40)
                .gain(0.05)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.40)
                        .sustain(0.0)
                        .release(0.0),
                ),
        )
        .layer(
            Oscillator::triangle(180.0)
                .start_at(0.025)
                .duration(0.18)
                .gain(0.05)
                .envelope(
                    Envelope::new()
                        .attack(0.01)
                        .decay(0.18)
                        .sustain(0.2)
                        .release(0.19),
                ),
        )
        .layer(Oscillator::sine(1100.0).duration(0.005).gain(0.08))
        .layer(
            Noise::white()
                .start_at(0.008)
                .duration(0.12)
                .low_pass(4200.0)
                .gain(0.20)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.15)
                        .sustain(0.50)
                        .release(0.07),
                ),
        )
}

fn enemy_shot(pitch: f32, seed: u32) -> Sound {
    let hz = |frequency: f32| frequency * pitch;
    Sound::duration_secs(0.18)
        .layer(
            Noise::white()
                .seed(seed)
                .duration(0.045)
                .low_pass(3200.0)
                .gain(0.42)
                .envelope(Envelope::fast_decay()),
        )
        .layer(
            Oscillator::sine(hz(190.0))
                .sweep_to(hz(74.0))
                .duration(0.15)
                .gain(0.36)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.14)
                        .sustain(0.0)
                        .release(0.0),
                ),
        )
        .layer(Oscillator::square(hz(880.0)).duration(0.01).gain(0.10))
        .layer(
            Oscillator::square(hz(420.0))
                .start_at(0.07)
                .duration(0.012)
                .gain(0.05),
        )
}

fn impact() -> Sound {
    Sound::duration_secs(0.20)
        .layer(
            Noise::white()
                .seed(21)
                .duration(0.03)
                .high_pass(1200.0)
                .gain(0.28)
                .envelope(Envelope::fast_decay()),
        )
        .layer(
            Oscillator::triangle(740.0).gain(0.32).envelope(
                Envelope::new()
                    .attack(0.001)
                    .decay(0.12)
                    .sustain(0.0)
                    .release(0.04),
            ),
        )
        .layer(
            Oscillator::triangle(1160.0).gain(0.14).envelope(
                Envelope::new()
                    .attack(0.001)
                    .decay(0.08)
                    .sustain(0.0)
                    .release(0.03),
            ),
        )
}

fn enemy_hit() -> Sound {
    Sound::duration_secs(0.24)
        .layer(
            Noise::white()
                .seed(27)
                .duration(0.08)
                .low_pass(2400.0)
                .gain(0.30)
                .envelope(Envelope::fast_decay()),
        )
        .layer(
            Oscillator::triangle(470.0)
                .sweep_to(160.0)
                .duration(0.20)
                .gain(0.28)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.16)
                        .sustain(0.0)
                        .release(0.04),
                ),
        )
}

fn player_hurt() -> Sound {
    Sound::duration_secs(0.42)
        .layer(
            Oscillator::sine(92.0).gain(0.40).envelope(
                Envelope::new()
                    .attack(0.005)
                    .decay(0.22)
                    .sustain(0.0)
                    .release(0.08),
            ),
        )
        .layer(
            Oscillator::sine(140.0).gain(0.16).envelope(
                Envelope::new()
                    .attack(0.0)
                    .decay(0.18)
                    .sustain(0.0)
                    .release(0.06),
            ),
        )
        .layer(
            Noise::white()
                .seed(33)
                .duration(0.18)
                .low_pass(1800.0)
                .gain(0.16)
                .envelope(
                    Envelope::new()
                        .attack(0.0)
                        .decay(0.12)
                        .sustain(0.0)
                        .release(0.04),
                ),
        )
}

fn death() -> Sound {
    Sound::duration_secs(0.80)
        .layer(
            Oscillator::sine(240.0)
                .sweep_to(48.0)
                .duration(0.70)
                .gain(0.34)
                .envelope(
                    Envelope::new()
                        .attack(0.01)
                        .decay(0.45)
                        .sustain(0.1)
                        .release(0.20),
                ),
        )
        .layer(
            Noise::white()
                .seed(41)
                .duration(0.55)
                .low_pass(1600.0)
                .gain(0.12)
                .envelope(
                    Envelope::new()
                        .attack(0.02)
                        .decay(0.30)
                        .sustain(0.2)
                        .release(0.18),
                ),
        )
        .layer(
            Oscillator::square(180.0)
                .start_at(0.04)
                .duration(0.03)
                .gain(0.08),
        )
}

fn menu_click() -> Sound {
    Sound::duration_secs(0.07)
        .layer(
            Oscillator::sine(680.0).gain(0.28).envelope(
                Envelope::new()
                    .attack(0.001)
                    .decay(0.04)
                    .sustain(0.0)
                    .release(0.015),
            ),
        )
        .layer(
            Oscillator::triangle(1020.0)
                .gain(0.10)
                .envelope(Envelope::fast_decay()),
        )
}

/// Eight seconds of machinery and wind. Every partial completes a whole number
/// of cycles, so the file can loop without a click.
fn ambience() -> Sound {
    Sound::duration_secs(8.0)
        .layer(Oscillator::sine(48.0).gain(0.16))
        .layer(Oscillator::sine(77.0).gain(0.09))
        .layer(Oscillator::sine(96.0).gain(0.04))
        .layer(Oscillator::sine(173.0).gain(0.035))
        .layer(Oscillator::sine(397.0).gain(0.025))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SAMPLE_RATE;
    use crate::synth::peak;

    #[test]
    fn the_example_renders_its_full_length() {
        let samples = example_steam_shot().render(SAMPLE_RATE);
        assert_eq!(samples.len(), 66_150);
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert!(peak(&samples) > 0.2);
    }

    #[test]
    fn exported_sounds_stay_audible_and_unclipped() {
        for item in exported() {
            let samples = item.sound.render(SAMPLE_RATE);
            let level = peak(&samples);
            assert!(
                (0.15..=1.0).contains(&level),
                "{} peaks at {level}",
                item.id
            );
            assert!(
                samples.iter().all(|sample| sample.is_finite()),
                "{}",
                item.id
            );
        }
    }

    #[test]
    fn ambience_joins_onto_its_first_sample() {
        let samples = ambience().render(SAMPLE_RATE);
        let join = (samples[0] - samples[samples.len() - 1]).abs();
        assert!(join < 0.05, "loop step {join}");
    }
}
