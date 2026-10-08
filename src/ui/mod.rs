//! Status text, health blocks, and the menus that interrupt play.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};

use crate::enemy::{EncounterDirector, WAVES_PER_PHASE};
use crate::player::{GunslingerWeapon, Player, PlayerStats};
use crate::session::{Phase, Session};

pub mod checkpoint;
pub mod pause;

#[derive(Resource, Clone)]
pub struct UiFonts {
    pub heading: Handle<Font>,
    pub body: Handle<Font>,
}

#[derive(Component)]
pub struct Hud;

#[derive(Component)]
pub struct HealthBlock(pub usize);

fn filled_health_blocks(current: u32, maximum: u32) -> usize {
    if maximum == 0 || current == 0 {
        return 0;
    }
    ((u64::from(current.min(maximum)) * 5).div_ceil(u64::from(maximum))) as usize
}

pub fn spawn_hud(commands: &mut Commands, fonts: &UiFonts) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font: FontSource::Handle(fonts.body.clone()),
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            top: px(16),
            ..default()
        },
        Hud,
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(20),
            top: px(54),
            height: px(20),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(5),
            ..default()
        })
        .with_children(|bar| {
            bar.spawn((
                Text::new("HEALTH"),
                TextFont {
                    font: FontSource::Handle(fonts.body.clone()),
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(Color::srgb(0.60, 0.82, 0.54)),
                Node {
                    margin: UiRect::right(px(8)),
                    ..default()
                },
            ));
            for index in 0..5 {
                bar.spawn((
                    HealthBlock(index),
                    Node {
                        width: px(33),
                        height: px(9),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.29, 0.82, 0.30)),
                    BorderColor::all(Color::srgb(0.20, 0.37, 0.18)),
                ));
            }
        });
    commands.spawn((
        Text::new("WASD move   •   Mouse aim   •   Left click fire   •   Esc pause"),
        TextFont {
            font: FontSource::Handle(fonts.body.clone()),
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.78, 0.86, 0.82)),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            bottom: px(16),
            ..default()
        },
    ));
}

pub fn update_hud(
    time: Res<Time>,
    mut session: ResMut<Session>,
    director: Res<EncounterDirector>,
    players: Query<&GunslingerWeapon, With<Player>>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    if session.phase == Phase::Playing {
        session.elapsed += time.delta_secs();
    }
    let (rounds, capacity) = players
        .iter()
        .next()
        .map(|weapon| (weapon.rounds, weapon.capacity))
        .unwrap_or((0, 0));
    if let Ok(mut text) = hud.single_mut() {
        let status = if session.phase == Phase::PlayerDead {
            "\n\nYOU DIED — Press R to restart"
        } else {
            ""
        };
        **text = format!(
            "PHASE {}   WAVE {}/{}   SLUGS {rounds}/{capacity}   KILLS {}   POINTS {}   TIME {:02}:{:02}{status}",
            director.phase_number,
            director.group_number,
            WAVES_PER_PHASE,
            session.kills,
            session.points,
            (session.elapsed as u32) / 60,
            (session.elapsed as u32) % 60,
        );
    }
}

pub fn update_health_bar(
    players: Query<(&crate::combat::Health, &PlayerStats), With<Player>>,
    mut blocks: Query<(&HealthBlock, &mut BackgroundColor)>,
) {
    let filled = players
        .iter()
        .next()
        .map(|(health, stats)| filled_health_blocks(health.0, stats.max_health))
        .unwrap_or(0);
    for (block, mut background) in &mut blocks {
        *background = BackgroundColor(if block.0 < filled {
            Color::srgb(0.29, 0.82, 0.30)
        } else {
            Color::BLACK
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_blocks_round_up_by_twenty_percent() {
        assert_eq!(filled_health_blocks(1000, 1000), 5);
        assert_eq!(filled_health_blocks(801, 1000), 5);
        assert_eq!(filled_health_blocks(800, 1000), 4);
        assert_eq!(filled_health_blocks(601, 1000), 4);
        assert_eq!(filled_health_blocks(600, 1000), 3);
        assert_eq!(filled_health_blocks(1, 1000), 1);
        assert_eq!(filled_health_blocks(0, 1000), 0);
        assert_eq!(filled_health_blocks(1500, 2000), 4);
    }
}
