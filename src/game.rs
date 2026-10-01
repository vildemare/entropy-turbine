use bevy::prelude::*;
use bevy::text::FontSize;

use crate::{
    character::CharacterAsset,
    combat::Projectile,
    enemy::{Enemy, SpawnClock},
    player::{self, Player},
};

pub const PLAYER_Y: f32 = 0.75;

#[derive(Resource, Default)]
pub struct Session {
    pub phase: Phase,
    pub kills: u32,
    pub elapsed: f32,
}

#[derive(Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Playing,
    PlayerDead,
}

#[derive(Resource, Clone)]
pub struct Visuals {
    pub enemy_mesh: Handle<Mesh>,
    pub enemy_material: Handle<StandardMaterial>,
    pub bullet_mesh: Handle<Mesh>,
    pub bullet_material: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct Hud;

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let visuals = Visuals {
        enemy_mesh: meshes.add(Cuboid::new(1.1, 1.1, 1.1)),
        enemy_material: materials.add(Color::srgb(0.47, 0.13, 0.10)),
        bullet_mesh: meshes.add(Sphere::new(0.17)),
        bullet_material: materials.add(Color::srgb(1.0, 0.58, 0.12)),
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
        Transform::from_xyz(0.0, 24.0, 21.0).looking_at(Vec3::ZERO, Vec3::Y),
        crate::camera::FollowCamera,
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
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
    commands.spawn((
        Text::new("WASD move   •   Mouse aim   •   Left click fire"),
        TextFont {
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
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
}

pub fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut clock: ResMut<SpawnClock>,
    mut commands: Commands,
    character: Res<CharacterAsset>,
    players: Query<Entity, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
    bullets: Query<Entity, With<Projectile>>,
) {
    if session.phase != Phase::PlayerDead || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in players.iter().chain(enemies.iter()).chain(bullets.iter()) {
        commands.entity(entity).despawn();
    }
    *session = Session::default();
    *clock = SpawnClock::default();
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
}

pub fn update_hud(
    time: Res<Time>,
    mut session: ResMut<Session>,
    players: Query<&crate::combat::Health, With<Player>>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    if session.phase == Phase::Playing {
        session.elapsed += time.delta_secs();
    }
    let health = players.iter().map(|health| health.0).max().unwrap_or(0);
    if let Ok(mut text) = hud.single_mut() {
        let status = if session.phase == Phase::PlayerDead {
            "\nYOU DIED — Press R to restart"
        } else {
            ""
        };
        **text = format!(
            "HEALTH  {health}     KILLS  {}     TIME  {:02}:{:02}{status}",
            session.kills,
            (session.elapsed as u32) / 60,
            (session.elapsed as u32) % 60,
        );
    }
}
