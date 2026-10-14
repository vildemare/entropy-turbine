//! Shared stepping, spacing, and the backup.
//!
//! The grenade walker and the cow take their steps here. Gunners call the
//! same backup while their cover gait runs. A body that is trying to move
//! and stops making progress backs up, then its caller resumes the route.

use std::time::Duration;

use bevy::{prelude::*, window::PrimaryWindow};

use super::Cover;
use super::grenadier::Grenadier;
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::route;

/// The spacing system noticed a body stop making progress.
/// The gunner peels its own strafe.
#[derive(Message, Clone, Copy, Debug)]
pub struct BackupStarted {
    pub body: Entity,
}

/// The launcher is aiming, so this step stays put and faces `face`.
#[derive(Message, Clone, Copy, Debug)]
pub struct WalkerHold {
    pub walker: Entity,
    pub face: Vec2,
}

/// Heading for the grenade walker's step. The step owns it.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct WalkerHeading(pub Vec2);

/// Shared backup when a walker or a cow stops making progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Unstick {
    pub stall: Duration,
    pub reverse: f32,
}

pub(crate) const UNSTICK: Unstick = Unstick {
    stall: Duration::from_millis(100),
    reverse: 1.2,
};

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
            if drift.stalled >= UNSTICK.stall.as_secs_f32() {
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
        if drift.stalled >= UNSTICK.stall.as_secs_f32() {
            drift.stalled = 0.0;
            drift.backing = UNSTICK.reverse;
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

/// A barricade or a rubble pile, in the shape a step checks against.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Footprint {
    pub center: Vec3,
    pub half_width: f32,
    pub half_depth: f32,
}

pub(crate) fn footprints<'a>(
    covers: impl IntoIterator<Item = (&'a Transform, &'a Cover)>,
) -> Vec<Footprint> {
    covers
        .into_iter()
        .map(|(transform, cover)| Footprint {
            center: transform.translation,
            half_width: cover.half_width,
            half_depth: cover.half_depth,
        })
        .collect()
}

/// Extra gap kept between the feet and a barricade, so the turn happens as
/// the walker meets the obstacle.
pub(crate) const WALKER_CLEARANCE: f32 = 0.4;

/// One step of the grenade walker's diagonal.
///
/// Meeting cover or the road edge turns the heading ninety degrees away from
/// that obstacle. `distance` is the pace for this frame.
pub(crate) fn step_walker(
    mut position: Vec3,
    mut heading: Vec2,
    radius: f32,
    distance: f32,
    height: f32,
    blocks: &[Footprint],
) -> (Vec3, Vec2) {
    let step = Vec3::new(heading.x, 0.0, heading.y) * distance;
    let proposed = position + step;
    if let Some(obstacle) = walker_blocked(proposed, radius, blocks) {
        heading = turn_away(heading, position, obstacle);
        let turned = position + Vec3::new(heading.x, 0.0, heading.y) * distance;
        if walker_blocked(turned, radius, blocks).is_none() {
            position = turned;
        }
    } else {
        position = proposed;
    }
    position.x = route::clamp_actor_x(position.x, position.z, radius);
    position.y = height;
    (position, heading)
}

pub(crate) fn walker_blocked(position: Vec3, radius: f32, blocks: &[Footprint]) -> Option<Vec3> {
    let reach = radius + WALKER_CLEARANCE;
    for block in blocks {
        if overlaps(position, reach, block) {
            return Some(block.center);
        }
    }
    let center = route::centerline_x(position.z);
    let limit = route::ROAD_HALF_WIDTH - radius;
    let local = position.x - center;
    if local.abs() > limit {
        let outside = center + local.signum() * (route::ROAD_HALF_WIDTH + 2.0);
        return Some(Vec3::new(outside, 0.0, position.z));
    }
    None
}

fn turn_away(heading: Vec2, from: Vec3, obstacle: Vec3) -> Vec2 {
    let to = Vec2::new(obstacle.x - from.x, obstacle.z - from.z);
    let cross = heading.x * to.y - heading.y * to.x;
    let left = Vec2::new(-heading.y, heading.x);
    let right = Vec2::new(heading.y, -heading.x);
    if cross.abs() < 0.05 {
        let here = Vec2::new(from.x, from.z);
        let obstacle = Vec2::new(obstacle.x, obstacle.z);
        if (here + left).distance_squared(obstacle) >= (here + right).distance_squared(obstacle) {
            left
        } else {
            right
        }
    } else if cross > 0.0 {
        right
    } else {
        left
    }
}

fn overlaps(position: Vec3, radius: f32, block: &Footprint) -> bool {
    let nearest_x = position.x.clamp(
        block.center.x - block.half_width,
        block.center.x + block.half_width,
    );
    let nearest_z = position.z.clamp(
        block.center.z - block.half_depth,
        block.center.z + block.half_depth,
    );
    let offset = Vec2::new(position.x - nearest_x, position.z - nearest_z);
    offset.length_squared() < radius * radius
}

/// One step of a cow along its lane, bending toward the clearer side of a
/// barricade that sits in the way.
pub(crate) fn step_tracks(
    position: Vec3,
    radius: f32,
    lane_x: f32,
    side: f32,
    distance: f32,
    blocks: &[Footprint],
) -> (Vec3, f32) {
    let Some(block) = obstacle_ahead(position, radius, blocks) else {
        let wander = (position.z * 0.45).sin() * 2.2;
        let lateral = (lane_x + wander - position.x).clamp(-6.0, 6.0);
        let forward = Vec3::new(lateral * 0.45, 0.0, 1.0).normalize();
        return (position + forward * distance, side);
    };
    let preferred = clearer_side(position, block, radius, side);
    for chosen in [preferred, -preferred] {
        let pass_x = route::clamp_actor_x(
            block.center.x + chosen * (block.half_width + radius + 1.1),
            block.center.z,
            radius,
        );
        let aim_z = (block.center.z - position.z).max(1.2);
        let direction = Vec3::new(pass_x - position.x, 0.0, aim_z).normalize();
        let next = position + direction * distance;
        if !tracks_blocked(next, radius, blocks) {
            return (next, chosen);
        }
    }
    (position, -side)
}

/// The nearest cover the cow would clip if it kept its current line, looked up
/// early enough that the side-step reads as a turn rather than a shove.
fn obstacle_ahead<'a>(
    position: Vec3,
    radius: f32,
    blocks: &'a [Footprint],
) -> Option<&'a Footprint> {
    let look = 14.0;
    let margin = radius + 1.4;
    blocks
        .iter()
        .filter(|block| {
            let front = block.center.z - block.half_depth;
            let back = block.center.z + block.half_depth;
            if back < position.z - 0.2 || front > position.z + look {
                return false;
            }
            let left = block.center.x - block.half_width - margin;
            let right = block.center.x + block.half_width + margin;
            position.x > left && position.x < right
        })
        .min_by(|a, b| {
            let da = a.center.z - a.half_depth - position.z;
            let db = b.center.z - b.half_depth - position.z;
            da.total_cmp(&db)
        })
}

fn clearer_side(position: Vec3, block: &Footprint, radius: f32, prefer: f32) -> f32 {
    let center = route::centerline_x(block.center.z);
    let left_room = (block.center.x - block.half_width) - (center - route::ROAD_HALF_WIDTH);
    let right_room = (center + route::ROAD_HALF_WIDTH) - (block.center.x + block.half_width);
    let prefer_right = if (position.x - block.center.x).abs() > 0.4 {
        position.x > block.center.x
    } else {
        prefer > 0.0
    };
    let prefer_room = if prefer_right { right_room } else { left_room };
    if prefer_room > radius + 0.8 {
        return if prefer_right { 1.0 } else { -1.0 };
    }
    if right_room >= left_room { 1.0 } else { -1.0 }
}

pub(crate) fn tracks_blocked(position: Vec3, radius: f32, blocks: &[Footprint]) -> bool {
    blocks.iter().any(|block| {
        let nearest_x = position.x.clamp(
            block.center.x - block.half_width,
            block.center.x + block.half_width,
        );
        let nearest_z = position.z.clamp(
            block.center.z - block.half_depth,
            block.center.z + block.half_depth,
        );
        let offset = Vec2::new(position.x - nearest_x, position.z - nearest_z);
        offset.length_squared() < radius * radius
    })
}

pub fn separate_enemies(
    time: Res<Time>,
    mut started: MessageWriter<BackupStarted>,
    mut enemies: Query<(Entity, &mut Transform, &mut super::Enemy), With<super::Enemy>>,
    covers: Query<(&Transform, &Cover), (With<Cover>, Without<super::Enemy>)>,
) {
    for _ in 0..2 {
        let mut pairs = enemies.iter_combinations_mut();
        while let Some([(_, mut a, enemy_a), (_, mut b, enemy_b)]) = pairs.fetch_next() {
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
    for (entity, mut transform, mut enemy) in &mut enemies {
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
            started.write(BackupStarted { body: entity });
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

pub fn walk_grenadiers(
    time: Res<Time>,
    session: Res<Session>,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<Grenadier>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    covers: Query<(&Transform, &Cover), Without<Grenadier>>,
    mut holds: MessageReader<WalkerHold>,
    mut walkers: Query<(Entity, &mut Transform, &mut WalkerHeading, &Grenadier), Without<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };
    let center_z = camera::framed_ground_z(camera_transform);
    let window = windows.iter().next();
    let exit_z = window
        .and_then(|window| {
            camera::ground_z_at_viewport_fraction(camera, camera_transform, window, 0.92)
        })
        .unwrap_or(center_z + 15.0);
    let blocks = footprints(covers.iter());
    let definition = crate::character::grenadier();
    let distance = definition.speed() * time.delta_secs();
    let height = definition.height();
    let holding: Vec<(Entity, Vec2)> = holds.read().map(|hold| (hold.walker, hold.face)).collect();

    for (entity, mut transform, mut heading, walker) in &mut walkers {
        if transform.translation.z > exit_z {
            continue;
        }
        if let Some((_, face)) = holding.iter().find(|(id, _)| *id == entity) {
            transform.rotation = Quat::from_rotation_y(face.x.atan2(face.y));
            continue;
        }
        let (position, next) = step_walker(
            transform.translation,
            heading.0,
            walker.radius,
            distance,
            height,
            &blocks,
        );
        heading.0 = next;
        transform.translation = position;
        transform.rotation = Quat::from_rotation_y(next.x.atan2(next.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(drift.backing, UNSTICK.reverse);
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
            backing: UNSTICK.reverse,
            stalled: 0.05,
            ..Drift::default()
        };
        assert!(!note_drift(&mut drift, Vec2::ZERO, 0.2));
        assert_eq!(drift.backing, UNSTICK.reverse);
        assert_eq!(drift.stalled, 0.0);
    }
}
