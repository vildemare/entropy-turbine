//! How a living enemy moves, keeps its distance, aims, and leaves the view.

use bevy::{prelude::*, window::PrimaryWindow};

use super::{AttackPhase, Cover, Enemy, EnemyKind, EnemyReward, EnemyState, Hull, Pace, Tactic};
use crate::audio::{self, SoundBank};
use crate::combat::{self, Damage, Delivery, Faction, Lifetime, Material, Projectile};
use crate::player::{MovementSpeed, Player};
use crate::session::{Phase, Session};
use crate::units::units;
use crate::world::camera::{self, FollowCamera};
use crate::world::route::{self, Boulder};
use crate::world::visuals::Visuals;

/// The metal round every rifle squad fires.
///
/// Speed and radius are units. 1000 units is 1 metre per second, and 1 metre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Round {
    pub speed: u32,
    pub radius: u32,
}

pub(crate) const RIFLE_ROUND: Round = Round {
    speed: 13_000,
    radius: 170,
};

/// One rifle: how hard it hits and how long it takes to load again.
/// The cartridge is shared. Reload is units. 1000 units is 1 second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rifle {
    pub damage: u32,
    pub reload: u32,
    pub round: Round,
}

/// A gunner is a hull, a pace, and a rifle. Nothing else is filled in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gunner {
    pub hull: Hull,
    pub pace: Pace,
    pub rifle: Rifle,
}

pub(crate) const SCOUT: Gunner = Gunner {
    hull: Hull {
        health: 3,
        reward: 1,
        radius: 440,
        height: 540,
        visual_scale: 0.80,
    },
    pace: Pace { speed: 3_700 },
    rifle: Rifle {
        damage: 150,
        reload: 1_150,
        round: RIFLE_ROUND,
    },
};

pub(crate) const TROOPER: Gunner = Gunner {
    hull: Hull {
        health: 6,
        reward: 2,
        radius: 560,
        height: 680,
        visual_scale: 1.0,
    },
    pace: Pace { speed: 3_000 },
    rifle: Rifle {
        damage: 200,
        reload: 1_550,
        round: RIFLE_ROUND,
    },
};

pub(crate) const HEAVY: Gunner = Gunner {
    hull: Hull {
        health: 15,
        reward: 4,
        radius: 730,
        height: 830,
        visual_scale: 1.22,
    },
    pace: Pace { speed: 2_100 },
    rifle: Rifle {
        damage: 300,
        reload: 1_950,
        round: RIFLE_ROUND,
    },
};

pub(crate) fn gunner(kind: EnemyKind) -> &'static Gunner {
    match kind {
        EnemyKind::Scout => &SCOUT,
        EnemyKind::Heavy => &HEAVY,
        EnemyKind::Trooper | EnemyKind::ZapBot | EnemyKind::Grenadier => &TROOPER,
    }
}

pub(crate) fn rifle(kind: EnemyKind) -> Rifle {
    gunner(kind).rifle
}

/// How far a rifle squad looks, and how many shots it fires before loading.
/// Sight is units. 1000 units is 1 metre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RifleDrill {
    pub sight: u32,
    pub burst: u8,
}

pub(crate) const RIFLE_DRILL: RifleDrill = RifleDrill {
    sight: 33_000,
    burst: 3,
};

/// Cover approach and the fall back.
/// Entry and exit are plain factors on the pace.
/// Retreat speed and duration are units. 1000 units is 1 metre per second, and 1 second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CoverGait {
    pub entry: f32,
    pub exit: f32,
    pub retreat_speed: u32,
    pub retreat_for: u32,
}

pub(crate) const COVER_GAIT: CoverGait = CoverGait {
    entry: 1.5,
    exit: 1.5,
    retreat_speed: 6_000,
    retreat_for: 3_500,
};

/// Shared backup when a walker or a cow stops making progress.
/// Stall and reverse are units. 1000 units is 1 second, and 1 metre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Unstick {
    pub stall: u32,
    pub reverse: u32,
}

pub(crate) const UNSTICK: Unstick = Unstick {
    stall: 100,
    reverse: 1_200,
};

#[derive(Component)]
pub struct Retreating {
    side: f32,
    rear_z: f32,
    remaining: f32,
}

/// Progress watchdog for a walker or a zap-bot.
///
/// While `trying` is set, a body that has not advanced for a tenth of a second
/// backs up along `-attempt`, then its caller resumes the normal step. A walker
/// that stands to shoot leaves `trying` clear, so the backup pauses and the
/// shot still happens.
#[derive(Clone, Copy, Debug)]
pub struct Drift {
    pub stalled: f32,
    pub backing: f32,
    pub attempt: Vec2,
    pub from: Vec3,
    pub step: Vec2,
    pub trying: bool,
    pub fresh: bool,
}

impl Default for Drift {
    fn default() -> Self {
        Self {
            stalled: 0.0,
            backing: 0.0,
            attempt: Vec2::ZERO,
            from: Vec3::ZERO,
            step: Vec2::ZERO,
            trying: false,
            fresh: false,
        }
    }
}

/// Step to apply instead of the normal one while a backup is underway.
/// `side` peels the reverse off the contact that stopped the body.
pub fn reverse_step(drift: &Drift, side: f32, max_step: f32) -> Option<Vec3> {
    if drift.backing <= 0.0 || max_step <= 0.0 {
        return None;
    }
    let back = if drift.attempt.length_squared() > 1.0e-6 {
        -drift.attempt.normalize()
    } else {
        Vec2::new(0.0, -1.0)
    };
    let lateral = Vec2::new(-back.y, back.x) * side.signum();
    let mut dir = back + lateral * 0.65;
    if dir.length_squared() < 1.0e-6 {
        dir = Vec2::new(0.0, -1.0);
    }
    let dir = dir.normalize();
    let distance = max_step.min(drift.backing);
    Some(Vec3::new(dir.x * distance, 0.0, dir.y * distance))
}

/// Record the XZ change after separation. Returns true when a backup begins,
/// so the caller can swap the side it will try next.
pub fn note_drift(drift: &mut Drift, moved: Vec2, dt: f32) -> bool {
    if !drift.fresh {
        return false;
    }
    drift.fresh = false;
    if !drift.trying {
        drift.stalled = 0.0;
        return false;
    }
    if drift.backing > 0.0 {
        let back = if drift.attempt.length_squared() > 1.0e-6 {
            -drift.attempt.normalize()
        } else {
            Vec2::new(0.0, -1.0)
        };
        let gained = moved.dot(back).max(0.0);
        drift.backing = (drift.backing - gained).max(0.0);
        if gained < progress_quota(drift.step) {
            drift.stalled += dt;
            if drift.stalled >= units(UNSTICK.stall) {
                drift.backing = 0.0;
                drift.stalled = 0.0;
            }
        } else {
            drift.stalled = 0.0;
        }
        return false;
    }
    let heading = if drift.step.length_squared() > 1.0e-8 {
        drift.step.normalize()
    } else if drift.attempt.length_squared() > 1.0e-8 {
        drift.attempt
    } else {
        Vec2::new(0.0, 1.0)
    };
    drift.attempt = heading;
    if moved.dot(heading) < progress_quota(drift.step) {
        drift.stalled += dt;
        if drift.stalled >= units(UNSTICK.stall) {
            drift.stalled = 0.0;
            drift.backing = units(UNSTICK.reverse);
            return true;
        }
    } else {
        drift.stalled = 0.0;
    }
    false
}

fn progress_quota(step: Vec2) -> f32 {
    if step.length() > 1.0e-4 {
        step.length() * 0.25
    } else {
        0.004
    }
}

pub fn move_enemies(
    time: Res<Time>,
    session: Res<Session>,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<
        (
            &mut Transform,
            &mut Enemy,
            &MovementSpeed,
            Option<&mut Retreating>,
        ),
        (With<Enemy>, Without<Player>),
    >,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
    for (mut transform, mut enemy, speed, retreating) in &mut enemies {
        let position = transform.translation;
        let target_player = players.iter().min_by(|a, b| {
            a.translation
                .distance_squared(position)
                .total_cmp(&b.translation.distance_squared(position))
        });
        match enemy.state {
            EnemyState::Entering => {
                let target = Vec3::new(enemy.slot_x, enemy.height, enemy.cover_z - 1.55);
                let max_step = speed.0 * COVER_GAIT.entry * dt;
                let desired = step_toward(transform.translation, target, max_step);
                travel(
                    &mut transform,
                    &mut enemy,
                    desired,
                    max_step,
                    desired.length_squared() > 1.0e-6,
                );
                if transform.translation.distance_squared(target) < 0.18 * 0.18 {
                    enemy.state = EnemyState::Holding;
                    enemy.drift.backing = 0.0;
                    enemy.drift.stalled = 0.0;
                    transform.scale.y = 0.58;
                    transform.translation.y = enemy.height * 0.58;
                }
            }
            EnemyState::Holding => {
                travel(&mut transform, &mut enemy, Vec3::ZERO, 0.0, false);
                enemy.hold_timer -= dt;
                if enemy.hold_timer <= 0.0 {
                    enemy.state = EnemyState::Flanking;
                    transform.scale.y = 1.0;
                    transform.translation.y = enemy.height;
                }
            }
            EnemyState::Flanking => {
                let target = Vec3::new(enemy.flank_x, enemy.height, enemy.cover_z - 1.55);
                let max_step = speed.0 * COVER_GAIT.exit * dt;
                let desired = step_toward(transform.translation, target, max_step);
                travel(
                    &mut transform,
                    &mut enemy,
                    desired,
                    max_step,
                    desired.length_squared() > 1.0e-6,
                );
                if transform.translation.distance_squared(target) < 0.18 * 0.18 {
                    enemy.state = EnemyState::Exiting;
                }
            }
            EnemyState::Exiting => {
                let max_step = speed.0 * COVER_GAIT.exit * dt;
                travel(
                    &mut transform,
                    &mut enemy,
                    Vec3::new(0.0, 0.0, max_step),
                    max_step,
                    true,
                );
                if transform.translation.z > enemy.cover_z + 1.7 {
                    enemy.state = EnemyState::Advancing;
                }
            }
            EnemyState::Advancing => {
                let Some(player) = target_player else {
                    travel(&mut transform, &mut enemy, Vec3::ZERO, 0.0, false);
                    continue;
                };
                let to_player = player.translation - transform.translation;
                let horizontal = Vec3::new(to_player.x, 0.0, to_player.z);
                let distance = horizontal.length();
                if distance > 0.01 {
                    transform.rotation = Quat::from_rotation_y(horizontal.x.atan2(horizontal.z));
                }
                if enemy.attack_phase == AttackPhase::Reloading || enemy.sight_blocked {
                    let forward = if distance > 8.0 { 0.5 } else { -0.1 };
                    let direction = Vec3::new(enemy.evade_direction, 0.0, forward).normalize();
                    let max_step = speed.0 * dt;
                    travel(
                        &mut transform,
                        &mut enemy,
                        direction * max_step,
                        max_step,
                        true,
                    );
                    if (transform.translation.x - route::centerline_x(transform.translation.z))
                        .abs()
                        > route::ROAD_HALF_WIDTH - enemy.radius - 0.5
                    {
                        enemy.evade_direction *= -1.0;
                    }
                } else if distance > units(RIFLE_DRILL.sight)
                    || distance > 5.5
                        && enemy.fire_timer > 0.0
                        && enemy.shots_left == RIFLE_DRILL.burst
                {
                    let direction = match enemy.tactic {
                        Tactic::Direct => horizontal.normalize_or_zero(),
                        Tactic::Straight => Vec3::Z,
                    };
                    let max_step = speed.0 * dt;
                    travel(
                        &mut transform,
                        &mut enemy,
                        direction * max_step,
                        max_step,
                        true,
                    );
                } else {
                    // In range with a shot ready: stand. A backup already
                    // underway waits, and shooting still runs on its own.
                    travel(&mut transform, &mut enemy, Vec3::ZERO, 0.0, false);
                }
            }
            EnemyState::Retreating => {
                if let Some(mut retreat) = retreating {
                    retreat.remaining -= dt;
                    transform.scale.y = 1.0;
                    transform.translation.y = enemy.height;
                    let max_step = units(COVER_GAIT.retreat_speed) * dt;
                    travel(
                        &mut transform,
                        &mut enemy,
                        Vec3::new(retreat.side * max_step, 0.0, 0.0),
                        max_step,
                        true,
                    );
                    transform.translation.z = transform.translation.z.min(retreat.rear_z);
                    transform.rotation =
                        Quat::from_rotation_y(retreat.side * std::f32::consts::FRAC_PI_2);
                }
            }
        }
        transform.translation.x = route::clamp_actor_x(
            transform.translation.x,
            transform.translation.z,
            enemy.radius,
        );
    }
}

fn travel(
    transform: &mut Transform,
    enemy: &mut Enemy,
    desired: Vec3,
    max_step: f32,
    trying: bool,
) {
    enemy.drift.from = transform.translation;
    enemy.drift.fresh = true;
    enemy.drift.trying = trying;
    if !trying {
        enemy.drift.step = Vec2::ZERO;
        return;
    }
    let step = reverse_step(&enemy.drift, enemy.evade_direction, max_step).unwrap_or(desired);
    enemy.drift.step = Vec2::new(step.x, step.z);
    transform.translation += step;
}

fn step_toward(position: Vec3, target: Vec3, max_step: f32) -> Vec3 {
    (target - position).clamp_length_max(max_step)
}

pub fn separate_enemies(
    time: Res<Time>,
    mut enemies: Query<(&mut Transform, &mut Enemy), With<Enemy>>,
    covers: Query<(&Transform, &Cover), (With<Cover>, Without<Enemy>)>,
) {
    for _ in 0..2 {
        let mut pairs = enemies.iter_combinations_mut();
        while let Some([(mut a, enemy_a), (mut b, enemy_b)]) = pairs.fetch_next() {
            let difference = Vec2::new(
                a.translation.x - b.translation.x,
                a.translation.z - b.translation.z,
            );
            let distance = difference.length();
            let minimum = enemy_a.radius + enemy_b.radius + 0.08;
            if distance >= minimum {
                continue;
            }
            let direction = if distance > 0.0001 {
                difference / distance
            } else {
                Vec2::X
            };
            let shift = direction * ((minimum - distance) * 0.5);
            a.translation.x =
                route::clamp_actor_x(a.translation.x + shift.x, a.translation.z, enemy_a.radius);
            a.translation.z += shift.y;
            b.translation.x =
                route::clamp_actor_x(b.translation.x - shift.x, b.translation.z, enemy_b.radius);
            b.translation.z -= shift.y;
        }
    }
    for (mut transform, mut enemy) in &mut enemies {
        for (cover_transform, cover) in &covers {
            separate_from_cover(
                &mut transform.translation,
                enemy.radius,
                cover_transform.translation,
                cover,
            );
        }
        transform.translation.x = route::clamp_actor_x(
            transform.translation.x,
            transform.translation.z,
            enemy.radius,
        );
        let moved = Vec2::new(
            transform.translation.x - enemy.drift.from.x,
            transform.translation.z - enemy.drift.from.z,
        );
        if note_drift(&mut enemy.drift, moved, time.delta_secs()) {
            enemy.evade_direction = -enemy.evade_direction;
        }
    }
}

pub fn separate_from_cover(position: &mut Vec3, radius: f32, center: Vec3, cover: &Cover) {
    let nearest_x = position
        .x
        .clamp(center.x - cover.half_width, center.x + cover.half_width);
    let nearest_z = position
        .z
        .clamp(center.z - cover.half_depth, center.z + cover.half_depth);
    let offset = Vec2::new(position.x - nearest_x, position.z - nearest_z);
    let distance = offset.length();
    if distance >= radius {
        return;
    }
    if distance > 0.0001 {
        let push = offset / distance * (radius - distance);
        position.x += push.x;
        position.z += push.y;
    } else {
        let left = position.x - (center.x - cover.half_width);
        let right = center.x + cover.half_width - position.x;
        let back = position.z - (center.z - cover.half_depth);
        let front = center.z + cover.half_depth - position.z;
        let nearest = left.min(right).min(back).min(front);
        if nearest == left {
            position.x = center.x - cover.half_width - radius;
        } else if nearest == right {
            position.x = center.x + cover.half_width + radius;
        } else if nearest == back {
            position.z = center.z - cover.half_depth - radius;
        } else {
            position.z = center.z + cover.half_depth + radius;
        }
    }
}

pub fn shoot_enemies(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    covers: Query<(&Transform, &Cover), Without<Enemy>>,
    boulders: Query<&Transform, (With<Boulder>, Without<Enemy>)>,
    mut enemy_queries: ParamSet<(
        Query<(Entity, &Transform, &Enemy)>,
        Query<(Entity, &mut Transform, &mut Enemy, Option<&EnemyReward>)>,
    )>,
    mut positions: Local<Vec<(Entity, Vec3, f32)>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    positions.clear();
    for (entity, transform, enemy) in enemy_queries.p0().iter() {
        positions.push((entity, transform.translation, enemy.radius));
    }
    for (entity, mut transform, mut enemy, reward) in enemy_queries.p1().iter_mut() {
        if enemy.state == EnemyState::Retreating {
            continue;
        }
        if enemy.state == EnemyState::Advancing && enemy.attack_phase == AttackPhase::Reloading {
            enemy.fire_timer -= time.delta_secs();
            if enemy.fire_timer <= 0.0 {
                enemy.attack_phase = AttackPhase::Burst;
                enemy.shots_left = RIFLE_DRILL.burst;
                enemy.fire_timer = 0.1;
            }
            continue;
        }
        let Some(player) = players.iter().min_by(|a, b| {
            a.translation
                .distance_squared(transform.translation)
                .total_cmp(&b.translation.distance_squared(transform.translation))
        }) else {
            continue;
        };
        let origin = Vec3::new(transform.translation.x, 0.75, transform.translation.z);
        let destination = Vec3::new(player.translation.x, 0.75, player.translation.z);
        let direction = (destination - origin).normalize_or_zero();
        if direction == Vec3::ZERO
            || origin.distance_squared(destination)
                > units(RIFLE_DRILL.sight) * units(RIFLE_DRILL.sight)
        {
            if enemy.state == EnemyState::Advancing {
                enemy.sight_blocked = false;
                enemy.fire_timer = enemy.fire_timer.min(0.3);
            }
            continue;
        }
        let start = origin + direction * (enemy.radius + 0.28);
        let blocked_by_ally = positions.iter().any(|(other, position, radius)| {
            *other != entity
                && combat::segment_circle_t(start, destination, *position, *radius + 0.15)
                    .is_some_and(|t| t < 0.98)
        });
        let blocked_by_cover = covers.iter().any(|(cover_transform, cover)| {
            combat::segment_cover_t(start, destination, cover_transform.translation, cover)
                .is_some_and(|t| t < 0.98)
        });
        let blocked_by_boulder = boulders.iter().any(|boulder| {
            combat::segment_boulder_t(start, destination, boulder).is_some_and(|t| t < 0.98)
        });
        let clear_sight = !blocked_by_ally && !blocked_by_cover && !blocked_by_boulder;
        if enemy.state != EnemyState::Advancing {
            if !clear_sight {
                continue;
            }
            // A clear view interrupts entry, sheltering, or the route around
            // cover. The first shot happens this frame.
            enemy.state = EnemyState::Advancing;
            enemy.attack_phase = AttackPhase::Burst;
            enemy.shots_left = RIFLE_DRILL.burst;
            enemy.fire_timer = 0.0;
            transform.scale.y = 1.0;
            transform.translation.y = enemy.height;
        }
        let aim = destination - origin;
        transform.rotation = Quat::from_rotation_y(aim.x.atan2(aim.z));
        enemy.sight_blocked = !clear_sight;
        if enemy.sight_blocked {
            enemy.fire_timer = enemy.fire_timer.min(0.18);
            continue;
        }
        enemy.fire_timer -= time.delta_secs();
        if enemy.fire_timer > 0.0 {
            continue;
        }
        let weapon = rifle(
            reward
                .map(|reward| reward.kind)
                .unwrap_or(EnemyKind::Trooper),
        );
        commands.spawn((
            Projectile {
                direction,
                speed: units(weapon.round.speed),
                radius: units(weapon.round.radius),
                previous: start,
                spent: false,
                faction: Faction::Enemy,
                material: Material::Metal,
                delivery: Delivery::Kinetic,
                owner: entity,
            },
            Damage(weapon.damage),
            Lifetime(2.5),
            Mesh3d(visuals.bullet_mesh.clone()),
            MeshMaterial3d(visuals.enemy_bullet_material.clone()),
            Transform::from_translation(start),
        ));
        let volume = (0.26 - origin.distance(destination) * 0.004).clamp(0.08, 0.26);
        let speed = 0.94 + (RIFLE_DRILL.burst - enemy.shots_left) as f32 * 0.04;
        audio::play_game(&mut commands, sounds.enemy_shot(), volume, speed);
        enemy.engaged = true;
        enemy.shots_left -= 1;
        if enemy.shots_left == 0 {
            enemy.attack_phase = AttackPhase::Reloading;
            enemy.fire_timer = enemy.reload_duration;
            enemy.evade_direction *= -1.0;
        } else {
            enemy.fire_timer = 0.38 + enemy.reload_duration * 0.06;
        }
    }
}

pub fn cleanup_behind_camera(
    mut commands: Commands,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<Enemy>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, Option<&Retreating>)>,
    covers: Query<(Entity, &Transform), (With<Cover>, Without<Enemy>)>,
    bullets: Query<(Entity, &Projectile)>,
) {
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };
    let enemy_cutoff = windows
        .iter()
        .next()
        .and_then(|window| {
            camera::ground_z_at_viewport_fraction(camera, camera_transform, window, 0.84)
        })
        .unwrap_or(camera_transform.translation.z - camera::CAMERA_BACK_OFFSET + 12.0);
    let cover_cutoff = camera_transform.translation.z - camera::CAMERA_BACK_OFFSET + 24.0;
    let mut removed = Vec::new();
    for (entity, mut transform, mut enemy, retreating) in &mut enemies {
        if let Some(retreat) = retreating {
            // Keep the retreat visible as the forward-only camera advances.
            transform.translation.z = transform.translation.z.min(enemy_cutoff - 1.0);
            if (transform.translation.x - route::centerline_x(transform.translation.z)).abs()
                >= route::ROAD_HALF_WIDTH - enemy.radius - 0.15
                || retreat.remaining <= 0.0
            {
                commands.entity(entity).despawn();
            }
            continue;
        }
        if transform.translation.z > enemy_cutoff {
            let local_x = transform.translation.x - route::centerline_x(transform.translation.z);
            let side = if local_x > 0.0 {
                1.0
            } else if local_x < 0.0 {
                -1.0
            } else if enemy.group_id.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            enemy.state = EnemyState::Retreating;
            transform.scale.y = 1.0;
            transform.translation.y = enemy.height;
            transform.translation.z = enemy_cutoff - 1.0;
            commands.entity(entity).insert(Retreating {
                side,
                rear_z: enemy_cutoff - 1.0,
                remaining: units(COVER_GAIT.retreat_for),
            });
            removed.push(entity);
        }
    }
    for (entity, projectile) in &bullets {
        if projectile.faction == Faction::Enemy && removed.contains(&projectile.owner) {
            commands.entity(entity).despawn();
        }
    }
    for (entity, transform) in &covers {
        if transform.translation.z > cover_cutoff {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn enemies_retreat_sideways_before_leaving_and_their_shots_stop() {
        let mut app = App::new();
        app.add_systems(Update, cleanup_behind_camera);
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        let enemy = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Advancing),
                Transform::from_xyz(0.0, 0.68, 13.0),
            ))
            .id();
        let bullet = app
            .world_mut()
            .spawn((Projectile {
                direction: Vec3::Z,
                speed: 16.0,
                radius: 0.17,
                previous: Vec3::ZERO,
                spent: false,
                faction: Faction::Enemy,
                material: Material::Metal,
                delivery: Delivery::Kinetic,
                owner: enemy,
            },))
            .id();
        let cover = app
            .world_mut()
            .spawn((
                Cover {
                    half_width: 2.7,
                    half_depth: 0.5,
                    height: 1.1,
                },
                Transform::from_xyz(0.0, 0.55, 13.0),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Enemy>(enemy).unwrap().state,
            EnemyState::Retreating
        );
        assert!(app.world().get::<Retreating>(enemy).is_some());
        assert!(app.world().get_entity(bullet).is_err());
        assert!(app.world().get_entity(cover).is_ok());
        app.world_mut()
            .entity_mut(enemy)
            .get_mut::<Transform>()
            .unwrap()
            .translation
            .x = route::ROAD_HALF_WIDTH - 0.56;
        app.update();
        assert!(app.world().get_entity(enemy).is_err());
    }
    #[test]
    fn retreating_enemy_does_not_fire_at_visible_player() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, shoot_enemies);
        let enemy = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Retreating),
                Transform::from_xyz(0.0, 0.68, -8.0),
            ))
            .id();
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.75, 0.0)));
        app.update();
        assert_eq!(
            app.world().get::<Enemy>(enemy).unwrap().state,
            EnemyState::Retreating
        );
        assert_eq!(
            app.world_mut()
                .query::<&Projectile>()
                .iter(app.world())
                .count(),
            0
        );
    }
    #[test]
    fn retreating_enemy_walks_toward_side_rocks() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .add_systems(Update, move_enemies);
        let enemy = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Retreating),
                Retreating {
                    side: 1.0,
                    rear_z: 8.0,
                    remaining: units(COVER_GAIT.retreat_for),
                },
                MovementSpeed(3.0),
                Transform::from_xyz(0.0, 0.68, 8.0),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        let position = app.world().get::<Transform>(enemy).unwrap().translation;
        assert!((position.x - 3.0).abs() < 0.01);
        assert_eq!(position.z, 8.0);
    }
    fn test_enemy(state: EnemyState) -> Enemy {
        Enemy {
            group_id: 1,
            engaged: false,
            state,
            tactic: Tactic::Direct,
            radius: 0.56,
            height: 0.68,
            cover_z: -23.0,
            slot_x: 0.0,
            flank_x: 4.0,
            hold_timer: 0.0,
            fire_timer: 0.8,
            attack_phase: AttackPhase::Burst,
            shots_left: RIFLE_DRILL.burst,
            evade_direction: 1.0,
            sight_blocked: false,
            reload_duration: 1.5,
            drift: Drift::default(),
        }
    }
    #[test]
    fn clear_sight_interrupts_cover_route_and_fires_immediately() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, shoot_enemies);
        app.world_mut().spawn((
            Cover {
                half_width: 2.7,
                half_depth: 0.5,
                height: 1.1,
            },
            Transform::from_xyz(0.0, 0.55, -23.0),
        ));
        let flanker = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Flanking),
                Transform::from_xyz(4.0, 0.68, -24.55),
            ))
            .id();
        let sheltered = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Holding),
                Transform::from_xyz(0.0, 0.4, -24.55).with_scale(Vec3::new(1.0, 0.58, 1.0)),
            ))
            .id();
        app.world_mut()
            .spawn((Player, Transform::from_xyz(4.0, 0.75, 0.0)));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();

        let flanker_state = app.world().get::<Enemy>(flanker).unwrap();
        assert_eq!(flanker_state.state, EnemyState::Advancing);
        assert_eq!(flanker_state.shots_left, RIFLE_DRILL.burst - 1);
        assert_eq!(
            app.world().get::<Enemy>(sheltered).unwrap().state,
            EnemyState::Holding
        );
        let bullets: Vec<_> = app
            .world_mut()
            .query::<&Projectile>()
            .iter(app.world())
            .map(|bullet| bullet.owner)
            .collect();
        assert_eq!(bullets, vec![flanker]);
    }
    #[test]
    fn gunner_fires_three_shots_then_moves_while_reloading() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, (shoot_enemies, move_enemies).chain());
        let gunner = app
            .world_mut()
            .spawn((
                Enemy {
                    group_id: 1,
                    engaged: false,
                    state: EnemyState::Advancing,
                    tactic: Tactic::Direct,
                    radius: 0.56,
                    height: 0.68,
                    cover_z: -10.0,
                    slot_x: 0.0,
                    flank_x: 0.0,
                    hold_timer: 0.0,
                    fire_timer: 0.0,
                    attack_phase: AttackPhase::Burst,
                    shots_left: RIFLE_DRILL.burst,
                    evade_direction: -1.0,
                    sight_blocked: false,
                    reload_duration: 1.5,
                    drift: Drift::default(),
                },
                MovementSpeed(3.0),
                Transform::from_xyz(0.0, 0.68, -10.0),
            ))
            .id();
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.75, 0.0)));

        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(500));
            app.update();
        }
        assert_eq!(
            app.world().get::<Enemy>(gunner).unwrap().attack_phase,
            AttackPhase::Reloading
        );
        assert_eq!(
            app.world_mut()
                .query::<&Projectile>()
                .iter(app.world())
                .count(),
            3
        );

        let x_before = app.world().get::<Transform>(gunner).unwrap().translation.x;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        let x_after = app.world().get::<Transform>(gunner).unwrap().translation.x;
        assert!(x_after > x_before);
        assert_eq!(
            app.world_mut()
                .query::<&Projectile>()
                .iter(app.world())
                .count(),
            3
        );
    }

    #[test]
    fn no_progress_for_a_tenth_of_a_second_starts_a_backup() {
        let mut drift = Drift {
            fresh: true,
            trying: true,
            step: Vec2::new(0.0, 0.3),
            ..Drift::default()
        };
        assert!(!note_drift(&mut drift, Vec2::ZERO, 0.05));
        assert!(drift.backing == 0.0);
        drift.fresh = true;
        assert!(note_drift(&mut drift, Vec2::ZERO, 0.05));
        assert_eq!(drift.backing, units(UNSTICK.reverse));
        assert_eq!(drift.attempt, Vec2::new(0.0, 1.0));
        let back = reverse_step(&drift, 1.0, 0.4).unwrap();
        assert!(
            back.z < -0.2,
            "backup should run back up the road, got {back}"
        );
    }

    #[test]
    fn a_real_step_clears_the_stall() {
        let mut drift = Drift {
            fresh: true,
            trying: true,
            stalled: 0.08,
            step: Vec2::new(0.0, 0.3),
            ..Drift::default()
        };
        assert!(!note_drift(&mut drift, Vec2::new(0.0, 0.3), 0.05));
        assert_eq!(drift.stalled, 0.0);
        assert_eq!(drift.backing, 0.0);
    }

    #[test]
    fn standing_to_shoot_pauses_a_backup() {
        let mut drift = Drift {
            fresh: true,
            trying: false,
            backing: units(UNSTICK.reverse),
            stalled: 0.05,
            ..Drift::default()
        };
        assert!(!note_drift(&mut drift, Vec2::ZERO, 0.2));
        assert_eq!(drift.backing, units(UNSTICK.reverse));
        assert_eq!(drift.stalled, 0.0);
    }

    #[test]
    fn a_walker_pinned_on_cover_backs_up_then_a_shot_still_stops_it() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(
                Update,
                (move_enemies, separate_enemies, shoot_enemies).chain(),
            );
        app.world_mut().spawn((
            Cover {
                half_width: 2.7,
                half_depth: 0.5,
                height: 1.1,
            },
            Transform::from_xyz(0.0, 0.55, 0.0),
        ));
        let walker = app
            .world_mut()
            .spawn((
                Enemy {
                    state: EnemyState::Exiting,
                    cover_z: 0.0,
                    ..test_enemy(EnemyState::Exiting)
                },
                MovementSpeed(3.0),
                Transform::from_xyz(0.0, 0.68, -0.7),
            ))
            .id();
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(50));
            app.update();
        }
        let pinned = app.world().get::<Transform>(walker).unwrap().translation.z;
        assert!(
            app.world().get::<Enemy>(walker).unwrap().drift.backing > 0.0,
            "a tenth of a second pinned on the cover should start the backup"
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(50));
        app.update();
        let backed = app.world().get::<Transform>(walker).unwrap().translation.z;
        assert!(
            backed < pinned - 0.05,
            "backup {backed} should be up the road from {pinned}"
        );

        let mut standing = test_enemy(EnemyState::Advancing);
        standing.drift.backing = units(UNSTICK.reverse);
        standing.drift.attempt = Vec2::new(0.0, 1.0);
        standing.fire_timer = 0.0;
        let shooter = app
            .world_mut()
            .spawn((
                standing,
                MovementSpeed(3.0),
                Transform::from_xyz(6.0, 0.68, -8.0),
            ))
            .id();
        app.world_mut()
            .spawn((Player, Transform::from_xyz(6.0, 0.75, -2.0)));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(50));
        app.update();
        let after = app.world().get::<Transform>(shooter).unwrap().translation;
        assert!(
            (after.z + 8.0).abs() < 0.02,
            "a ready shot should hold the walker, z {}",
            after.z
        );
        assert_eq!(
            app.world().get::<Enemy>(shooter).unwrap().drift.backing,
            units(UNSTICK.reverse)
        );
        assert!(
            app.world_mut()
                .query::<&Projectile>()
                .iter(app.world())
                .any(|shot| shot.owner == shooter),
            "the walker still fires during the backup"
        );
    }
}
