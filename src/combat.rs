use bevy::prelude::*;

use crate::{
    enemy::Enemy,
    game::{Phase, Session},
    player::Player,
    street,
};

const BULLET_SPEED: f32 = 34.0;

#[derive(Component)]
pub struct Health(pub i32);

#[derive(Component)]
pub struct Damage(pub i32);

#[derive(Component)]
pub struct Lifetime(pub f32);

#[derive(Component)]
pub struct Projectile {
    pub direction: Vec3,
    pub previous: Vec3,
    pub spent: bool,
}

#[derive(Component, Default)]
pub struct ContactCooldown(pub f32);

pub fn move_projectiles(
    time: Res<Time>,
    session: Res<Session>,
    mut bullets: Query<(&mut Transform, &mut Projectile)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut transform, mut bullet) in &mut bullets {
        bullet.previous = transform.translation;
        transform.translation += bullet.direction * BULLET_SPEED * time.delta_secs();
    }
}

pub fn hit_enemies(
    session: ResMut<Session>,
    mut bullets: Query<(&Transform, &mut Projectile, &Damage)>,
    mut enemies: Query<(Entity, &Transform, &mut Health), With<Enemy>>,
    mut commands: Commands,
) {
    let mut session = session;
    if session.phase != Phase::Playing {
        return;
    }
    for (bullet_transform, mut bullet, damage) in &mut bullets {
        if bullet.spent {
            continue;
        }
        let segment = bullet_transform.translation - bullet.previous;
        for (entity, enemy_transform, mut health) in &mut enemies {
            if health.0 <= 0 {
                continue;
            }
            let t = if segment.length_squared() > 0.0 {
                ((enemy_transform.translation - bullet.previous).dot(segment)
                    / segment.length_squared())
                .clamp(0.0, 1.0)
            } else {
                0.0
            };
            let nearest = bullet.previous + segment * t;
            if nearest.distance_squared(enemy_transform.translation) > 0.75 * 0.75 {
                continue;
            }
            bullet.spent = true;
            health.0 -= damage.0;
            if health.0 <= 0 {
                commands.entity(entity).despawn();
                session.kills += 1;
            }
            break;
        }
    }
}

pub fn contact_damage(
    time: Res<Time>,
    mut session: ResMut<Session>,
    enemies: Query<&Transform, With<Enemy>>,
    mut players: Query<(&Transform, &mut Health, &mut ContactCooldown), With<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (transform, mut health, mut cooldown) in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
        if cooldown.0 > 0.0 {
            continue;
        }
        if enemies
            .iter()
            .any(|enemy| enemy.translation.distance_squared(transform.translation) < 1.1 * 1.1)
        {
            health.0 -= 1;
            cooldown.0 = 0.8;
            if health.0 <= 0 {
                session.phase = Phase::PlayerDead;
            }
        }
    }
}

pub fn expire_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut bullets: Query<(Entity, &Transform, &mut Lifetime, &Projectile)>,
) {
    for (entity, transform, mut lifetime, bullet) in &mut bullets {
        lifetime.0 -= time.delta_secs();
        if lifetime.0 <= 0.0
            || bullet.spent
            || transform.translation.x.abs() >= street::ROAD_HALF_WIDTH
        {
            commands.entity(entity).despawn();
        }
    }
}
