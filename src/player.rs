use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    character::{CharacterAsset, CharacterModel},
    combat::{ContactCooldown, Damage, Health, Lifetime, Projectile},
    game::{PLAYER_Y, Phase, Session, Visuals},
    street,
};

const MOVE_SPEED: f32 = 8.0;
const FIRE_INTERVAL: f32 = 0.14;
pub const SHOOT_SPEED_FACTOR: f32 = 2.0 / 3.0;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct MovementSpeed(pub f32);

#[derive(Component, Default)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub aim: Vec3,
    pub firing: bool,
}

#[derive(Component)]
pub struct FireCooldown(pub f32);

pub fn spawn_player(commands: &mut Commands, character: &CharacterAsset, position: Vec3) {
    let actor = commands
        .spawn((
            Player,
            LocalPlayer,
            PlayerIntent::default(),
            MovementSpeed(MOVE_SPEED),
            FireCooldown(0.0),
            ContactCooldown::default(),
            Health(5),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id();
    commands.spawn((
        CharacterModel,
        WorldAssetRoot(character.scene.clone()),
        Transform::from_xyz(0.0, -PLAYER_Y, 0.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            .with_scale(Vec3::splat(0.8)),
        ChildOf(actor),
    ));
}

pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    mut players: Query<&mut PlayerIntent, (With<Player>, With<LocalPlayer>)>,
) {
    let mut movement = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8) as f32,
        (keys.pressed(KeyCode::KeyS) as i8 - keys.pressed(KeyCode::KeyW) as i8) as f32,
    );
    movement = movement.normalize_or_zero();
    let aim = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            let (camera, transform) = camera.single().ok()?;
            let ray = camera.viewport_to_world(transform, cursor).ok()?;
            let direction = *ray.direction;
            if direction.y.abs() < 0.0001 {
                return None;
            }
            let distance = -ray.origin.y / direction.y;
            (distance > 0.0).then_some(ray.origin + direction * distance)
        });

    for mut intent in &mut players {
        intent.movement = movement;
        intent.firing = mouse.pressed(MouseButton::Left);
        if let Some(aim) = aim {
            intent.aim = aim;
        }
    }
}

pub fn move_players(
    time: Res<Time>,
    session: Res<Session>,
    mut players: Query<(&mut Transform, &PlayerIntent, &MovementSpeed), With<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut transform, intent, speed) in &mut players {
        let speed_factor = if intent.firing {
            SHOOT_SPEED_FACTOR
        } else {
            1.0
        };
        transform.translation += Vec3::new(intent.movement.x, 0.0, intent.movement.y)
            * speed.0
            * speed_factor
            * time.delta_secs();
        transform.translation.x = street::clamp_actor_x(transform.translation.x, 0.9);
    }
}

pub fn face_players(
    session: Res<Session>,
    mut players: Query<(&mut Transform, &PlayerIntent), With<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut transform, intent) in &mut players {
        let direction = if intent.movement.length_squared() > 0.001 {
            Vec3::new(intent.movement.x, 0.0, intent.movement.y)
        } else {
            let mut direction = intent.aim - transform.translation;
            direction.y = 0.0;
            direction
        };
        if direction.length_squared() > 0.001 {
            transform.look_to(direction, Vec3::Y);
        }
    }
}

pub fn shoot(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut commands: Commands,
    mut players: Query<(&Transform, &PlayerIntent, &mut FireCooldown), With<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (transform, intent, mut cooldown) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if !intent.firing || cooldown.0 > 0.0 {
            continue;
        }
        let mut direction = intent.aim - transform.translation;
        direction.y = 0.0;
        let direction = direction.normalize_or_zero();
        if direction == Vec3::ZERO {
            continue;
        }
        cooldown.0 = FIRE_INTERVAL;
        commands.spawn((
            Projectile {
                direction,
                previous: transform.translation + direction * 0.95,
                spent: false,
            },
            Damage(1),
            Lifetime(1.4),
            Mesh3d(visuals.bullet_mesh.clone()),
            MeshMaterial3d(visuals.bullet_material.clone()),
            Transform::from_translation(transform.translation + direction * 0.95),
        ));
    }
}
