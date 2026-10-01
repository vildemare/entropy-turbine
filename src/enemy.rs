use bevy::prelude::*;

use crate::{
    combat::Health,
    game::{Phase, Session, Visuals},
    player::{MovementSpeed, Player},
    street,
};

const MAX_ENEMIES: usize = 100;

#[derive(Component)]
pub struct Enemy;

#[derive(Resource)]
pub struct SpawnClock {
    timer: f32,
    random_state: u32,
}

impl Default for SpawnClock {
    fn default() -> Self {
        Self {
            timer: 0.8,
            random_state: 0x8b45_73cf,
        }
    }
}

impl SpawnClock {
    fn random(&mut self) -> f32 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 17;
        self.random_state ^= self.random_state << 5;
        self.random_state as f32 / u32::MAX as f32
    }
}

pub fn spawn_enemies(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut clock: ResMut<SpawnClock>,
    mut commands: Commands,
    players: Query<&Transform, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
) {
    if session.phase != Phase::Playing || enemies.iter().len() >= MAX_ENEMIES {
        return;
    }
    clock.timer -= time.delta_secs();
    if clock.timer > 0.0 {
        return;
    }
    clock.timer = (0.85 - session.elapsed * 0.006).max(0.22);
    let Some(player) = players.iter().next() else {
        return;
    };
    let x = (clock.random() * 2.0 - 1.0) * (street::ROAD_HALF_WIDTH - 1.0);
    let direction = if clock.random() < 0.75 { -1.0 } else { 1.0 };
    let z = player.translation.z + direction * (32.0 + clock.random() * 8.0);
    let position = Vec3::new(x, 0.55, z);
    commands.spawn((
        Enemy,
        Health(3),
        MovementSpeed(2.8 + (session.elapsed * 0.008).min(1.5)),
        Mesh3d(visuals.enemy_mesh.clone()),
        MeshMaterial3d(visuals.enemy_material.clone()),
        Transform::from_translation(position),
    ));
}

pub fn chase_players(
    time: Res<Time>,
    session: Res<Session>,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&mut Transform, &MovementSpeed), (With<Enemy>, Without<Player>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut enemy, speed) in &mut enemies {
        let target = players.iter().min_by(|a, b| {
            a.translation
                .distance_squared(enemy.translation)
                .total_cmp(&b.translation.distance_squared(enemy.translation))
        });
        let Some(target) = target else { continue };
        let mut direction = target.translation - enemy.translation;
        direction.y = 0.0;
        let direction = direction.normalize_or_zero();
        enemy.translation += direction * speed.0 * time.delta_secs();
        enemy.translation.x = street::clamp_actor_x(enemy.translation.x, 0.8);
    }
}
