//! Builds the first scene and restarts it after death. Frame order lives in `main`.

use bevy::prelude::*;

use crate::audio;
use crate::combat::{PointOrb, Projectile};
use crate::enemy::{Cover, EncounterDirector, Enemy};
use crate::player::animation::CharacterAsset;
use crate::player::{self, PLAYER_Y, Player};
use crate::session::{Phase, Session};
use crate::ui::checkpoint::CheckpointLine;
use crate::ui::{self, UiFonts, pause};
use crate::world::camera::{CAMERA_BACK_OFFSET, CAMERA_HEIGHT, FollowCamera};
use crate::world::route;
use crate::world::visuals;

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
    audio::setup(&mut commands, &asset_server);
    commands.insert_resource(visuals::load(&mut meshes, &mut materials));
    let character = CharacterAsset::load(&asset_server);
    commands.insert_resource(character.clone());
    route::setup(&mut commands, &mut meshes, &mut materials);
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
        Transform::from_xyz(0.0, CAMERA_HEIGHT, CAMERA_BACK_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
        FollowCamera,
    ));
    ui::spawn_hud(&mut commands, &fonts);
    pause::spawn(&mut commands, &fonts);
    ui::checkpoint::spawn(&mut commands, &fonts);
    player::ammo_hud::spawn(&mut commands, &fonts);
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
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
    mut cameras: Query<&mut Transform, With<crate::world::camera::FollowCamera>>,
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
            crate::world::camera::CAMERA_HEIGHT,
            crate::world::camera::CAMERA_BACK_OFFSET,
        );
    }
    player::spawn_player(&mut commands, &character, Vec3::new(0.0, PLAYER_Y, 0.0));
}
