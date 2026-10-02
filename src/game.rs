use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use bevy::time::Virtual;

use crate::{
    character::CharacterAsset,
    combat::{PointOrb, Projectile},
    enemy::{Cover, EncounterDirector, Enemy, WAVES_PER_PHASE},
    player::{
        self, GunslingerWeapon, HEALTH_UPGRADE, MAX_CAPACITY, MAX_HEALTH, MIN_RELOAD_WAIT_MS,
        Player, PlayerProgression, PlayerStats, reload_wait_ms_for,
    },
    sound::{self, SoundBank},
};

pub const PLAYER_Y: f32 = 0.75;

#[derive(Resource, Default)]
pub struct Session {
    pub phase: Phase,
    pub resume_phase: Phase,
    pub suppress_fire_until_release: bool,
    pub kills: u32,
    pub points: u32,
    pub elapsed: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Playing,
    Paused,
    Checkpoint,
    PlayerDead,
}

#[derive(Resource, Clone)]
pub struct UiFonts {
    pub heading: Handle<Font>,
    pub body: Handle<Font>,
}

#[derive(Resource, Clone, Default)]
pub struct Visuals {
    pub enemy_part_mesh: Handle<Mesh>,
    pub scout_material: Handle<StandardMaterial>,
    pub trooper_material: Handle<StandardMaterial>,
    pub heavy_material: Handle<StandardMaterial>,
    pub enemy_iron_material: Handle<StandardMaterial>,
    pub enemy_brass_material: Handle<StandardMaterial>,
    pub enemy_visor_material: Handle<StandardMaterial>,
    pub bullet_mesh: Handle<Mesh>,
    pub bullet_material: Handle<StandardMaterial>,
    pub enemy_bullet_material: Handle<StandardMaterial>,
    pub barricade_mesh: Handle<Mesh>,
    pub barricade_material: Handle<StandardMaterial>,
    pub rubble_mesh: Handle<Mesh>,
    pub rubble_material: Handle<StandardMaterial>,
    pub orb_mesh: Handle<Mesh>,
    pub orb_material: Handle<StandardMaterial>,
    pub checkpoint_mesh: Handle<Mesh>,
    pub checkpoint_material: Handle<StandardMaterial>,
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

#[derive(Component)]
pub struct PauseOverlay;

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
            let available =
                (stats.max_health < MAX_HEALTH).then(|| upgrade_price(progression.health_level));
            (
                if available.is_some() {
                    format!(
                        "MAX HEALTH   {} → {}",
                        stats.max_health,
                        stats.max_health + HEALTH_UPGRADE
                    )
                } else {
                    format!("MAX HEALTH   {}", stats.max_health)
                },
                available,
            )
        }
        UpgradeKind::Capacity => {
            let available =
                (stats.capacity < MAX_CAPACITY).then(|| upgrade_price(progression.capacity_level));
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
            let available = (stats.reload_wait_ms > MIN_RELOAD_WAIT_MS)
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

#[derive(Component, Clone, Copy)]
pub enum PauseAction {
    Continue,
    Exit,
}

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let fonts = UiFonts {
        heading: asset_server.load("fonts/Cinzel.ttf"),
        body: asset_server.load("fonts/Oxanium.ttf"),
    };
    commands.insert_resource(fonts.clone());
    sound::setup(&mut commands, &asset_server);
    let visuals = Visuals {
        enemy_part_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        scout_material: materials.add(Color::srgb(0.40, 0.22, 0.12)),
        trooper_material: materials.add(Color::srgb(0.24, 0.27, 0.25)),
        heavy_material: materials.add(Color::srgb(0.15, 0.17, 0.19)),
        enemy_iron_material: materials.add(Color::srgb(0.12, 0.13, 0.14)),
        enemy_brass_material: materials.add(Color::srgb(0.58, 0.39, 0.13)),
        enemy_visor_material: materials.add(Color::srgb(0.60, 0.14, 0.07)),
        bullet_mesh: meshes.add(Sphere::new(0.17)),
        bullet_material: materials.add(Color::srgb(1.0, 0.58, 0.12)),
        enemy_bullet_material: materials.add(Color::srgb(0.88, 0.19, 0.07)),
        barricade_mesh: meshes.add(Cuboid::new(5.4, 1.1, 1.0)),
        barricade_material: materials.add(Color::srgb(0.20, 0.17, 0.13)),
        rubble_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        rubble_material: materials.add(Color::srgb(0.27, 0.23, 0.18)),
        orb_mesh: meshes.add(Sphere::new(0.32)),
        orb_material: materials.add(Color::srgb(0.95, 0.65, 0.18)),
        checkpoint_mesh: meshes.add(Cuboid::new(25.0, 0.08, 0.4)),
        checkpoint_material: materials.add(Color::srgb(0.76, 0.55, 0.22)),
    };
    commands.insert_resource(visuals.clone());
    let character = CharacterAsset::load(&asset_server);
    commands.insert_resource(character.clone());
    crate::street::setup(&mut commands, &mut meshes, &mut materials);
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 5200.0,
            color: Color::srgb(1.0, 0.82, 0.66),
            ..default()
        },
        Transform::from_xyz(8.0, 18.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.78, 0.72, 0.66),
        brightness: 260.0,
        ..default()
    });

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            0.0,
            crate::camera::CAMERA_HEIGHT,
            crate::camera::CAMERA_BACK_OFFSET,
        )
        .looking_at(Vec3::ZERO, Vec3::Y),
        crate::camera::FollowCamera,
    ));
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
    spawn_pause_menu(&mut commands, &fonts);
    spawn_checkpoint_menu(&mut commands, &fonts);
    crate::ammo_hud::spawn(&mut commands, &fonts);
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
}

fn spawn_pause_menu(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.014, 0.012, 0.78)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(430),
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(15),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.09, 0.075)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("ENTROPY TURBINE"),
                        TextFont {
                            font: FontSource::Handle(fonts.heading.clone()),
                            font_size: FontSize::Px(34.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.81, 0.65, 0.39)),
                    ));
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.71, 0.71, 0.65)),
                    ));
                    pause_button(panel, fonts, PauseAction::Continue, "CONTINUE");
                    pause_button(panel, fonts, PauseAction::Exit, "EXIT GAME");
                    panel.spawn((
                        Text::new("ESC  TO CONTINUE"),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.55, 0.53, 0.47)),
                    ));
                });
        });
}

fn spawn_checkpoint_menu(commands: &mut Commands, fonts: &UiFonts) {
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
    camera: Query<&Transform, With<crate::camera::FollowCamera>>,
    lines: Query<Entity, With<CheckpointLine>>,
) {
    if session.phase != Phase::Playing || !lines.is_empty() {
        return;
    }
    let (Some(z), Some(camera)) = (director.checkpoint_z, camera.iter().next()) else {
        return;
    };
    let center = camera.translation.z - crate::camera::CAMERA_BACK_OFFSET;
    if z < center - 45.0 {
        return;
    }
    commands.spawn((
        CheckpointLine,
        Mesh3d(visuals.checkpoint_mesh.clone()),
        MeshMaterial3d(visuals.checkpoint_material.clone()),
        Transform::from_xyz(0.0, 0.07, z),
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
            &mut crate::combat::Health,
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
        sound::play_ui(&mut commands, &sounds.menu_click);
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
            &mut crate::combat::Health,
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
        sound::play_ui(&mut commands, &sounds.menu_click);
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

fn pause_button(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    action: PauseAction,
    label: &str,
) {
    panel
        .spawn((
            Button,
            action,
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
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(fonts.body.clone()),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.84, 0.64)),
            ));
        });
}

pub fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    sound::play_ui(&mut commands, &sounds.menu_click);
    if session.phase == Phase::Paused {
        session.phase = session.resume_phase;
        session.suppress_fire_until_release = true;
        if session.phase == Phase::Checkpoint {
            time.pause();
        } else {
            time.unpause();
        }
    } else {
        session.resume_phase = session.phase;
        session.phase = Phase::Paused;
        time.pause();
    }
}

pub fn handle_pause_buttons(
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    mut exit: MessageWriter<AppExit>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut buttons: Query<
        (&Interaction, &PauseAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, action, mut background) in &mut buttons {
        *background = BackgroundColor(match interaction {
            Interaction::None => Color::srgb(0.24, 0.20, 0.14),
            Interaction::Hovered => Color::srgb(0.34, 0.27, 0.17),
            Interaction::Pressed => Color::srgb(0.48, 0.34, 0.17),
        });
        if session.phase != Phase::Paused || *interaction != Interaction::Pressed {
            continue;
        }
        sound::play_ui(&mut commands, &sounds.menu_click);
        match action {
            PauseAction::Continue => {
                session.phase = session.resume_phase;
                session.suppress_fire_until_release = true;
                if session.phase == Phase::Checkpoint {
                    time.pause();
                } else {
                    time.unpause();
                }
            }
            PauseAction::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub fn sync_pause_menu(
    session: Res<Session>,
    mut overlay: Query<&mut Visibility, With<PauseOverlay>>,
) {
    if !session.is_changed() {
        return;
    }
    for mut visibility in &mut overlay {
        *visibility = if session.phase == Phase::Paused {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut director: ResMut<EncounterDirector>,
    mut commands: Commands,
    character: Res<CharacterAsset>,
    players: Query<Entity, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
    bullets: Query<Entity, With<Projectile>>,
    orbs: Query<Entity, With<PointOrb>>,
    covers: Query<Entity, With<Cover>>,
    lines: Query<Entity, With<CheckpointLine>>,
    mut cameras: Query<&mut Transform, With<crate::camera::FollowCamera>>,
) {
    if session.phase != Phase::PlayerDead || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in players
        .iter()
        .chain(enemies.iter())
        .chain(bullets.iter())
        .chain(orbs.iter())
        .chain(covers.iter())
        .chain(lines.iter())
    {
        commands.entity(entity).despawn();
    }
    *session = Session::default();
    *director = EncounterDirector::default();
    for mut camera in &mut cameras {
        camera.translation = Vec3::new(
            0.0,
            crate::camera::CAMERA_HEIGHT,
            crate::camera::CAMERA_BACK_OFFSET,
        );
    }
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
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

    #[test]
    fn escape_pauses_and_resumes_game_time() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(Time::<Virtual>::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, (toggle_pause, sync_pause_menu).chain());
        let overlay = app
            .world_mut()
            .spawn((PauseOverlay, Visibility::Hidden))
            .id();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Paused);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            app.world().get::<Visibility>(overlay),
            Some(&Visibility::Visible)
        );

        *app.world_mut().resource_mut::<ButtonInput<KeyCode>>() = ButtonInput::default();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
        assert!(
            app.world()
                .resource::<Session>()
                .suppress_fire_until_release
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            app.world().get::<Visibility>(overlay),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn pause_buttons_continue_and_request_exit() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Paused,
            resume_phase: Phase::Playing,
            ..default()
        })
        .insert_resource(Time::<Virtual>::default())
        .insert_resource(SoundBank::default())
        .add_message::<AppExit>()
        .add_systems(Update, handle_pause_buttons);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().spawn((
            Button,
            PauseAction::Continue,
            Interaction::Pressed,
            BackgroundColor(Color::BLACK),
        ));
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
        assert!(
            app.world()
                .resource::<Session>()
                .suppress_fire_until_release
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());

        app.world_mut().resource_mut::<Session>().phase = Phase::Paused;
        app.world_mut().spawn((
            Button,
            PauseAction::Exit,
            Interaction::Pressed,
            BackgroundColor(Color::BLACK),
        ));
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<AppExit>>()
                .drain()
                .next(),
            Some(AppExit::Success)
        );
    }
}
