//! The brass crossing line and the shop that opens after a phase.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use bevy::time::Virtual;

use crate::audio::{self, SoundBank};
use crate::combat::{Health, Projectile};
use crate::enemy::{EncounterDirector, Enemy};
use crate::player::{
    self, GunslingerWeapon, PLAYER_Y, Player, PlayerProgression, PlayerStats, reload_wait_ms_for,
};
use crate::session::{Phase, Session};
use crate::tuning::{GUNSLINGER_PLAYER, GUNSLINGER_WEAPON};
use crate::ui::UiFonts;
use crate::world::camera::{CAMERA_BACK_OFFSET, FollowCamera};
use crate::world::route;
use crate::world::visuals::Visuals;

#[derive(Component)]
pub struct CheckpointOverlay;

#[derive(Component)]
pub struct CheckpointSummary;

#[derive(Component)]
pub struct CheckpointLine;

#[derive(Component)]
pub struct CheckpointContinue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeKind {
    Health,
    Capacity,
    Recharge,
}

#[derive(Component)]
pub struct ShopPurchase(pub UpgradeKind);

#[derive(Component)]
pub struct ShopLabel(pub UpgradeKind);

const PRICE_STEPS: [u32; 8] = [10, 12, 14, 18, 24, 32, 42, 50];

fn upgrade_price(purchases: u32) -> u32 {
    PRICE_STEPS[(purchases as usize).min(PRICE_STEPS.len() - 1)]
}

fn seconds_label(milliseconds: u32) -> String {
    format!("{}.{:03}s", milliseconds / 1000, milliseconds % 1000)
}

fn offer(kind: UpgradeKind, progression: &PlayerProgression) -> (String, Option<u32>) {
    let stats = progression.stats();
    match kind {
        UpgradeKind::Health => {
            let available = (stats.max_health < GUNSLINGER_PLAYER.max_health)
                .then(|| upgrade_price(progression.health_level));
            (
                if available.is_some() {
                    format!(
                        "MAX HEALTH   {} → {}",
                        stats.max_health,
                        stats.max_health + GUNSLINGER_PLAYER.health_per_upgrade
                    )
                } else {
                    format!("MAX HEALTH   {}", stats.max_health)
                },
                available,
            )
        }
        UpgradeKind::Capacity => {
            let available = (stats.capacity < GUNSLINGER_WEAPON.max_capacity)
                .then(|| upgrade_price(progression.capacity_level));
            (
                if available.is_some() {
                    format!(
                        "SLUG CAPACITY   {} → {}",
                        stats.capacity,
                        stats.capacity + 1
                    )
                } else {
                    format!("SLUG CAPACITY   {}", stats.capacity)
                },
                available,
            )
        }
        UpgradeKind::Recharge => {
            let available = (stats.reload_wait_ms > GUNSLINGER_WEAPON.min_reload_wait_ms)
                .then(|| upgrade_price(progression.reload_level));
            let next = reload_wait_ms_for(progression.reload_level + 1);
            (
                if available.is_some() {
                    format!(
                        "QUICKLOAD   {} → {}",
                        seconds_label(stats.reload_wait_ms),
                        seconds_label(next)
                    )
                } else {
                    format!("QUICKLOAD   {}", seconds_label(stats.reload_wait_ms))
                },
                available,
            )
        }
    }
}

pub fn spawn(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            CheckpointOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.014, 0.012, 0.82)),
            GlobalZIndex(90),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(460),
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(18),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.09, 0.075)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("CHECKPOINT SHOP"),
                        TextFont {
                            font: FontSource::Handle(fonts.heading.clone()),
                            font_size: FontSize::Px(34.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.81, 0.65, 0.39)),
                    ));
                    panel.spawn((
                        CheckpointSummary,
                        Text::new(""),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(21.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.85, 0.72)),
                    ));
                    for kind in [
                        UpgradeKind::Health,
                        UpgradeKind::Capacity,
                        UpgradeKind::Recharge,
                    ] {
                        shop_button(panel, fonts, kind);
                    }
                    panel
                        .spawn((
                            Button,
                            CheckpointContinue,
                            Node {
                                width: percent(100),
                                height: px(50),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.24, 0.20, 0.14)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("CONTINUE TO NEXT PHASE"),
                                TextFont {
                                    font: FontSource::Handle(fonts.body.clone()),
                                    font_size: FontSize::Px(21.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.94, 0.84, 0.64)),
                            ));
                        });
                });
        });
}

fn shop_button(panel: &mut ChildSpawnerCommands, fonts: &UiFonts, kind: UpgradeKind) {
    panel
        .spawn((
            Button,
            ShopPurchase(kind),
            Node {
                width: percent(100),
                height: px(56),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.20, 0.14)),
        ))
        .with_children(|button| {
            button.spawn((
                ShopLabel(kind),
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(fonts.body.clone()),
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.84, 0.64)),
            ));
        });
}

pub fn place_checkpoint_line(
    session: Res<Session>,
    director: Res<EncounterDirector>,
    visuals: Res<Visuals>,
    mut commands: Commands,
    camera: Query<&Transform, With<FollowCamera>>,
    lines: Query<Entity, With<CheckpointLine>>,
) {
    if session.phase != Phase::Playing || !lines.is_empty() {
        return;
    }
    let (Some(z), Some(camera)) = (director.checkpoint_z, camera.iter().next()) else {
        return;
    };
    let center = camera.translation.z - CAMERA_BACK_OFFSET;
    if z < center - 45.0 {
        return;
    }
    commands.spawn((
        CheckpointLine,
        Mesh3d(visuals.checkpoint_mesh.clone()),
        MeshMaterial3d(visuals.checkpoint_material.clone()),
        Transform::from_xyz(route::centerline_x(z), 0.07, z),
    ));
}

pub fn cross_checkpoint(
    mut session: ResMut<Session>,
    director: Res<EncounterDirector>,
    mut time: ResMut<Time<Virtual>>,
    mut commands: Commands,
    players: Query<&Transform, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
    bullets: Query<Entity, With<Projectile>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some(z) = director.checkpoint_z else {
        return;
    };
    if players.iter().any(|player| player.translation.z <= z) {
        for entity in enemies.iter().chain(bullets.iter()) {
            commands.entity(entity).despawn();
        }
        session.phase = Phase::Checkpoint;
        session.suppress_fire_until_release = true;
        time.pause();
    }
}

pub fn handle_checkpoint_button(
    mut session: ResMut<Session>,
    mut director: ResMut<EncounterDirector>,
    mut time: ResMut<Time<Virtual>>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    lines: Query<Entity, With<CheckpointLine>>,
    mut players: Query<
        (
            &mut Health,
            &PlayerProgression,
            &mut PlayerStats,
            &mut GunslingerWeapon,
        ),
        With<Player>,
    >,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<CheckpointContinue>),
    >,
) {
    for (interaction, mut background) in &mut buttons {
        *background = BackgroundColor(match interaction {
            Interaction::None => Color::srgb(0.24, 0.20, 0.14),
            Interaction::Hovered => Color::srgb(0.34, 0.27, 0.17),
            Interaction::Pressed => Color::srgb(0.48, 0.34, 0.17),
        });
        if session.phase != Phase::Checkpoint || *interaction != Interaction::Pressed {
            continue;
        }
        audio::play_ui(&mut commands, &sounds.menu_click);
        for line in &lines {
            commands.entity(line).despawn();
        }
        for (mut health, progression, mut stats, mut weapon) in &mut players {
            player::apply_progression(*progression, &mut stats, &mut health, &mut weapon, true);
        }
        director.next_phase();
        session.phase = Phase::Playing;
        session.suppress_fire_until_release = true;
        time.unpause();
    }
}

pub fn handle_shop_buttons(
    mut session: ResMut<Session>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut players: Query<
        (
            &mut Health,
            &mut PlayerProgression,
            &mut PlayerStats,
            &mut GunslingerWeapon,
        ),
        With<Player>,
    >,
    buttons: Query<(&Interaction, &ShopPurchase), (Changed<Interaction>, With<Button>)>,
) {
    if session.phase != Phase::Checkpoint {
        return;
    }
    let Ok((mut health, mut progression, mut stats, mut weapon)) = players.single_mut() else {
        return;
    };
    for (interaction, purchase) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (_, Some(price)) = offer(purchase.0, &progression) else {
            continue;
        };
        if session.points < price {
            continue;
        }
        session.points -= price;
        audio::play_ui(&mut commands, &sounds.menu_click);
        match purchase.0 {
            UpgradeKind::Health => progression.health_level += 1,
            UpgradeKind::Capacity => progression.capacity_level += 1,
            UpgradeKind::Recharge => progression.reload_level += 1,
        }
        player::apply_progression(*progression, &mut stats, &mut health, &mut weapon, false);
    }
}

pub fn sync_checkpoint_menu(
    session: Res<Session>,
    director: Res<EncounterDirector>,
    mut overlay: Query<&mut Visibility, With<CheckpointOverlay>>,
    mut summary: Query<&mut Text, (With<CheckpointSummary>, Without<ShopLabel>)>,
    players: Query<&PlayerProgression, With<Player>>,
    mut labels: Query<(&ShopLabel, &mut Text), Without<CheckpointSummary>>,
    mut buttons: Query<(&ShopPurchase, &Interaction, &mut BackgroundColor)>,
) {
    for mut visibility in &mut overlay {
        *visibility = if session.phase == Phase::Checkpoint {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut text in &mut summary {
        **text = format!(
            "PHASE {} CLEARED\nKILLS  {}     POINTS  {}",
            director.phase_number, session.kills, session.points
        );
    }
    let Ok(progression) = players.single() else {
        return;
    };
    for (label, mut text) in &mut labels {
        let (description, price) = offer(label.0, progression);
        **text = match price {
            Some(price) => format!("{description}     {price} P"),
            None => format!("{description}     MAXED"),
        };
    }
    for (purchase, interaction, mut background) in &mut buttons {
        let (_, price) = offer(purchase.0, progression);
        let affordable = price.is_some_and(|price| session.points >= price);
        *background = BackgroundColor(if !affordable {
            Color::srgb(0.14, 0.13, 0.11)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.34, 0.27, 0.17)
        } else if *interaction == Interaction::Pressed {
            Color::srgb(0.48, 0.34, 0.17)
        } else {
            Color::srgb(0.24, 0.20, 0.14)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_continue_restores_upgraded_maximum_health() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Checkpoint,
            ..default()
        })
        .insert_resource(EncounterDirector::default())
        .insert_resource(Time::<Virtual>::default())
        .insert_resource(SoundBank::default())
        .add_systems(Update, handle_checkpoint_button);
        let player = app
            .world_mut()
            .spawn((
                Player,
                crate::combat::Health(150),
                PlayerProgression {
                    health_level: 1,
                    ..default()
                },
                PlayerProgression {
                    health_level: 1,
                    ..default()
                }
                .stats(),
                GunslingerWeapon::default(),
            ))
            .id();
        app.world_mut().spawn((
            Button,
            CheckpointContinue,
            Interaction::Pressed,
            BackgroundColor(Color::BLACK),
        ));
        app.update();
        assert_eq!(
            app.world().get::<crate::combat::Health>(player).unwrap().0,
            1200
        );
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
        assert_eq!(app.world().resource::<EncounterDirector>().phase_number, 2);
    }
    #[test]
    fn shop_prices_climb_and_stop_at_fifty() {
        assert_eq!(
            (0..10).map(upgrade_price).collect::<Vec<_>>(),
            vec![10, 12, 14, 18, 24, 32, 42, 50, 50, 50]
        );
    }
    #[test]
    fn checkpoint_purchase_spends_points_and_improves_health() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Checkpoint,
            points: 24,
            ..default()
        })
        .insert_resource(SoundBank::default())
        .add_systems(Update, handle_shop_buttons);
        let player = app
            .world_mut()
            .spawn((
                Player,
                crate::combat::Health(600),
                PlayerProgression { ..default() },
                PlayerProgression::default().stats(),
                GunslingerWeapon::default(),
            ))
            .id();
        app.world_mut().spawn((
            Button,
            ShopPurchase(UpgradeKind::Health),
            Interaction::Pressed,
        ));
        app.update();
        let session = app.world().resource::<Session>();
        assert_eq!(session.points, 14);
        assert_eq!(
            app.world()
                .get::<PlayerProgression>(player)
                .unwrap()
                .health_level,
            1
        );
        assert_eq!(
            offer(
                UpgradeKind::Capacity,
                app.world().get::<PlayerProgression>(player).unwrap()
            )
            .1,
            Some(10)
        );
        assert_eq!(
            app.world().get::<crate::combat::Health>(player).unwrap().0,
            800
        );
        assert_eq!(
            app.world().get::<PlayerStats>(player).unwrap().max_health,
            1200
        );
        app.update();
        assert_eq!(app.world().resource::<Session>().points, 14);
    }
    #[test]
    fn checkpoint_purchases_build_all_three_stats_from_separate_levels() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Checkpoint,
            points: 30,
            ..default()
        })
        .insert_resource(SoundBank::default())
        .add_systems(Update, handle_shop_buttons);
        let player = app
            .world_mut()
            .spawn((
                Player,
                crate::combat::Health(500),
                PlayerProgression::default(),
                PlayerProgression::default().stats(),
                GunslingerWeapon::default(),
            ))
            .id();
        for kind in [
            UpgradeKind::Health,
            UpgradeKind::Capacity,
            UpgradeKind::Recharge,
        ] {
            app.world_mut()
                .spawn((Button, ShopPurchase(kind), Interaction::Pressed));
        }
        app.update();
        assert_eq!(app.world().resource::<Session>().points, 0);
        let build = app.world().get::<PlayerProgression>(player).unwrap();
        assert_eq!(
            (build.health_level, build.capacity_level, build.reload_level),
            (1, 1, 1)
        );
        assert_eq!(
            *app.world().get::<PlayerStats>(player).unwrap(),
            PlayerStats {
                max_health: 1200,
                capacity: 7,
                reload_wait_ms: 975
            }
        );
        assert_eq!(
            app.world().get::<crate::combat::Health>(player).unwrap().0,
            700
        );
    }
    #[test]
    fn checkpoint_shop_displays_next_price_and_disables_unaffordable_offer() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Checkpoint,
            points: 11,
            ..default()
        })
        .insert_resource(EncounterDirector::default())
        .add_systems(Update, sync_checkpoint_menu);
        app.world_mut().spawn((
            Player,
            PlayerProgression {
                health_level: 1,
                ..default()
            },
        ));
        app.world_mut()
            .spawn((CheckpointOverlay, Visibility::Hidden));
        app.world_mut().spawn((CheckpointSummary, Text::new("")));
        let label = app
            .world_mut()
            .spawn((ShopLabel(UpgradeKind::Health), Text::new("")))
            .id();
        let button = app
            .world_mut()
            .spawn((
                ShopPurchase(UpgradeKind::Health),
                Interaction::None,
                BackgroundColor(Color::BLACK),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Text>(label).unwrap().contains("12 P"));
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(0.14, 0.13, 0.11)
        );
    }
    #[test]
    fn crossing_checkpoint_stops_combat_and_clears_old_attackers() {
        let mut app = App::new();
        let mut director = EncounterDirector::default();
        director.checkpoint_z = Some(-10.0);
        app.insert_resource(Session::default())
            .insert_resource(director)
            .insert_resource(Time::<Virtual>::default())
            .add_systems(Update, cross_checkpoint);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, PLAYER_Y, -11.0)));
        let enemy = app
            .world_mut()
            .spawn(Enemy {
                group_id: 0,
                engaged: false,
                state: crate::enemy::EnemyState::Advancing,
                tactic: crate::enemy::Tactic::Direct,
                radius: 0.5,
                height: 0.5,
                cover_z: 0.0,
                slot_x: 0.0,
                flank_x: 0.0,
                hold_timer: 0.0,
                fire_timer: 0.0,
                attack_phase: crate::enemy::AttackPhase::Burst,
                shots_left: 3,
                evade_direction: 1.0,
                sight_blocked: false,
                reload_duration: 1.0,
            })
            .id();
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Checkpoint);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert!(app.world().get_entity(enemy).is_err());
    }
}
