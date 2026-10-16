use std::sync::atomic::{AtomicU32, Ordering};

use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};

use crate::session::{Phase, Session};

#[derive(Resource, Default)]
pub struct SoundBank {
    pub player_shots: [Handle<AudioSource>; 3],
    pub enemy_shots: [Handle<AudioSource>; 3],
    pub impact: Handle<AudioSource>,
    pub enemy_hit: Handle<AudioSource>,
    pub armor_hit: Handle<AudioSource>,
    pub shield_hit: Handle<AudioSource>,
    pub player_hurt: Handle<AudioSource>,
    pub zap_charge: Handle<AudioSource>,
    pub zap: Handle<AudioSource>,
    pub grenade_lob: Handle<AudioSource>,
    pub grenade_blast: Handle<AudioSource>,
    pub empty_click: Handle<AudioSource>,
    pub cylinder_full: Handle<AudioSource>,
    pub enemy_blast: Handle<AudioSource>,
    pub shield_break: Handle<AudioSource>,
    pub death: Handle<AudioSource>,
    pub menu_click: Handle<AudioSource>,
    pub ambience: Handle<AudioSource>,
    variant_counter: AtomicU32,
}

impl SoundBank {
    fn next_variant(&self) -> usize {
        let mut value = self
            .variant_counter
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(0x9e37_79b9);
        value ^= value >> 16;
        value = value.wrapping_mul(0x7feb_352d);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846c_a68b);
        ((value ^ (value >> 16)) as usize) % 3
    }

    pub fn player_shot(&self) -> &Handle<AudioSource> {
        &self.player_shots[self.next_variant()]
    }

    pub fn enemy_shot(&self) -> &Handle<AudioSource> {
        &self.enemy_shots[self.next_variant()]
    }
}

#[derive(Component)]
pub struct GameplayAudio;

pub fn setup(commands: &mut Commands, assets: &AssetServer) {
    let sounds = SoundBank {
        player_shots: [
            assets.load("audio/player_shot.wav"),
            assets.load("audio/player_shot_2.wav"),
            assets.load("audio/player_shot_3.wav"),
        ],
        enemy_shots: [
            assets.load("audio/enemy_shot.wav"),
            assets.load("audio/enemy_shot_2.wav"),
            assets.load("audio/enemy_shot_3.wav"),
        ],
        impact: assets.load("audio/impact.wav"),
        enemy_hit: assets.load("audio/enemy_hit.wav"),
        armor_hit: assets.load("audio/armor_hit.wav"),
        shield_hit: assets.load("audio/shield_hit.wav"),
        player_hurt: assets.load("audio/player_hurt.wav"),
        zap_charge: assets.load("audio/zap_charge.wav"),
        zap: assets.load("audio/zap.wav"),
        grenade_lob: assets.load("audio/grenade_lob.wav"),
        grenade_blast: assets.load("audio/grenade_blast.wav"),
        empty_click: assets.load("audio/empty_click.wav"),
        cylinder_full: assets.load("audio/cylinder_full.wav"),
        enemy_blast: assets.load("audio/enemy_blast.wav"),
        shield_break: assets.load("audio/shield_break.wav"),
        death: assets.load("audio/death.wav"),
        menu_click: assets.load("audio/menu_click.wav"),
        ambience: assets.load("audio/ambience.wav"),
        variant_counter: AtomicU32::new(0),
    };
    commands.spawn((
        AudioPlayer::new(sounds.ambience.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.10)),
        GameplayAudio,
    ));
    commands.insert_resource(sounds);
}

pub fn play_game(commands: &mut Commands, clip: &Handle<AudioSource>, volume: f32, speed: f32) {
    commands.spawn((
        AudioPlayer::new(clip.clone()),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_speed(speed),
        GameplayAudio,
    ));
}

pub fn play_ui(commands: &mut Commands, clip: &Handle<AudioSource>) {
    commands.spawn((
        AudioPlayer::new(clip.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.28)),
    ));
}

pub fn sync_gameplay_audio(session: Res<Session>, sinks: Query<&AudioSink, With<GameplayAudio>>) {
    for sink in &sinks {
        if session.phase == Phase::Paused || session.phase == Phase::Title {
            if !sink.is_paused() {
                sink.pause();
            }
        } else if sink.is_paused() {
            sink.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::audio::Decodable;

    use super::*;

    #[test]
    fn shot_variants_cover_the_whole_bank() {
        let sounds = SoundBank::default();
        let variants: std::collections::HashSet<_> =
            (0..24).map(|_| sounds.next_variant()).collect();
        assert_eq!(variants, [0, 1, 2].into());
    }

    #[test]
    fn every_bundled_sound_decodes() {
        for name in [
            "player_shot.wav",
            "player_shot_2.wav",
            "player_shot_3.wav",
            "enemy_shot.wav",
            "enemy_shot_2.wav",
            "enemy_shot_3.wav",
            "impact.wav",
            "enemy_hit.wav",
            "armor_hit.wav",
            "shield_hit.wav",
            "player_hurt.wav",
            "zap_charge.wav",
            "zap.wav",
            "grenade_lob.wav",
            "grenade_blast.wav",
            "empty_click.wav",
            "cylinder_full.wav",
            "enemy_blast.wav",
            "shield_break.wav",
            "death.wav",
            "menu_click.wav",
            "ambience.wav",
        ] {
            let path = format!("{}/assets/audio/{name}", env!("CARGO_MANIFEST_DIR"));
            let bytes = std::fs::read(path).unwrap();
            let source = AudioSource {
                bytes: bytes.into(),
            };
            assert!(
                source.decoder().next().is_some(),
                "{name} contains no samples"
            );
        }
    }
}
