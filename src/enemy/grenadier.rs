//! A wide-footed walker that lobs a sparse volley of grenades.
//!
//! It walks a diagonal down the road. Meeting cover or the road edge turns it
//! ninety degrees away from that obstacle, and it keeps walking. A player
//! inside its reach stops it: it turns toward that player, aims for two
//! seconds, then throws four grenades around the aim point. The aim point is
//! the center of the pattern. Reload is one second per grenade, and the walker
//! moves during that reload. Each landing is a circle on the ground; damage
//! falls off from the center to the rim.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::{Cover, EnemyKind, EnemyReward};
use crate::ability::Struck;
use crate::audio::{self, SoundBank};
use crate::character::{CharacterDefinition, LauncherSpec};
use crate::combat::{Delivery, EnergyShield, Faction, Health, HitCircle, Material};
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::route;
use crate::world::visuals::Visuals;

fn blueprint() -> &'static CharacterDefinition {
    crate::character::grenadier()
}

fn launcher() -> &'static LauncherSpec {
    blueprint().launcher()
}

/// Extra gap kept between the feet and a barricade, so the turn happens as
/// the walker meets the obstacle.
pub(crate) const WALKER_CLEARANCE: f32 = 0.4;

#[derive(Component)]
pub struct Grenadier {
    pub radius: f32,
    heading: Vec2,
    gait: Gait,
    rng: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gait {
    Walking,
    Aiming(f32),
    Reloading(f32),
}

struct Block {
    center: Vec3,
    half_width: f32,
    half_depth: f32,
}

struct Advance {
    position: Vec3,
    rotation: Quat,
    volley: Option<[Vec3; 4]>,
}

/// A grenade in flight, or the mark on the ground it is falling toward.
#[derive(Component)]
pub struct VolleyPiece;

#[derive(Component)]
pub(crate) struct Lob {
    origin: Vec3,
    target: Vec3,
    age: f32,
    owner: Entity,
    mark: Entity,
}

#[derive(Component)]
pub(crate) struct BlastMark {
    spin: f32,
}

pub(crate) fn spawn(
    commands: &mut Commands,
    visuals: &Visuals,
    position: Vec3,
    side: f32,
    rng: u32,
) {
    let hull = blueprint();
    let (shield, down_for) = hull.shield().expect("grenadier has a shield");
    let heading = Vec2::new(side * 0.65, 1.0).normalize();
    let actor = commands
        .spawn((
            Grenadier {
                radius: hull.radius(),
                heading,
                gait: Gait::Walking,
                rng: rng | 1,
            },
            EnemyReward {
                kind: EnemyKind::Grenadier,
                hit_player: false,
            },
            Health(hull.health()),
            EnergyShield::full(shield, down_for.as_secs_f32()),
            hull.runtime_abilities(),
            HitCircle {
                radius: hull.radius(),
                faction: Faction::Enemy,
                swell: true,
            },
            Transform::from_translation(position)
                .with_rotation(Quat::from_rotation_y(heading.x.atan2(heading.y))),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    spawn_visual(commands, visuals, actor, hull.visual_scale());
}

pub fn walk_grenadiers(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<Grenadier>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    covers: Query<(&Transform, &Cover), Without<Grenadier>>,
    mut walkers: Query<(Entity, &mut Transform, &mut Grenadier), Without<Player>>,
    players: Query<&Transform, (With<Player>, Without<Grenadier>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };
    let center_z = camera_transform.translation.z - camera::CAMERA_BACK_OFFSET;
    let window = windows.iter().next();
    let exit_z = window
        .and_then(|window| {
            camera::ground_z_at_viewport_fraction(camera, camera_transform, window, 0.92)
        })
        .unwrap_or(center_z + 15.0);
    let blocks = cover_blocks(&covers);
    let player_positions: Vec<Vec3> = players
        .iter()
        .map(|transform| transform.translation)
        .collect();
    let dt = time.delta_secs();

    for (entity, mut transform, mut walker) in &mut walkers {
        if transform.translation.z > exit_z {
            commands.entity(entity).despawn();
            continue;
        }
        let step = advance(
            &mut walker,
            transform.translation,
            &player_positions,
            &blocks,
            dt,
        );
        transform.translation = step.position;
        transform.rotation = step.rotation;
        if let Some(spots) = step.volley {
            audio::play_game(&mut commands, &sounds.grenade_lob, 0.28, 1.0);
            let origin = Vec3::new(step.position.x, step.position.y + 0.35, step.position.z);
            for spot in spots {
                spawn_throw(&mut commands, &visuals, entity, origin, spot);
            }
        }
    }
}

pub fn fly_grenades(
    time: Res<Time>,
    session: Res<Session>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut strikes: MessageWriter<Struck>,
    mut lobs: Query<(Entity, &mut Transform, &mut Lob)>,
    mut marks: Query<(&mut Transform, &mut BlastMark), (Without<Lob>, Without<Health>)>,
    bodies: Query<(Entity, &Transform, &Health), Without<Lob>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
    let mut pending = Vec::new();
    for (mut transform, mut mark) in &mut marks {
        mark.spin += dt * 2.4;
        transform.rotation = Quat::from_rotation_y(mark.spin);
    }
    for (entity, mut transform, mut lob) in &mut lobs {
        lob.age += dt;
        let weapon = launcher();
        let flight = weapon.flight.as_secs_f32();
        let t = (lob.age / flight).clamp(0.0, 1.0);
        let flat = lob.origin.lerp(lob.target, t);
        let height = 4.0 * weapon.arc_peak * t * (1.0 - t);
        transform.translation = Vec3::new(flat.x, lob.target.y + 0.35 + height, flat.z);
        if lob.age < flight {
            continue;
        }
        audio::play_game(&mut commands, &sounds.grenade_blast, 0.16, 1.0);
        for (body, body_transform, health) in &bodies {
            sting(
                body,
                body_transform.translation,
                health.0,
                lob.target,
                lob.owner,
                &mut pending,
            );
        }
        commands.entity(lob.mark).despawn();
        commands.entity(entity).despawn();
    }
    // A volley lands together. The strongest sting for a body is written first,
    // so the player's grace answers that one.
    pending.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then(right.damage.cmp(&left.damage))
    });
    for strike in pending {
        strikes.write(strike);
    }
}

fn sting(
    entity: Entity,
    position: Vec3,
    health: u32,
    center: Vec3,
    owner: Entity,
    pending: &mut Vec<Struck>,
) {
    if health == 0 {
        return;
    }
    let distance = Vec2::new(position.x - center.x, position.z - center.z).length();
    let weapon = launcher();
    let damage = blast_damage(distance, weapon.blast_radius, weapon.blast_damage);
    if damage == 0 {
        return;
    }
    pending.push(Struck {
        target: entity,
        source: owner,
        damage,
        material: Material::Explosive,
        delivery: Delivery::Kinetic,
        faction: Faction::Enemy,
    });
}

pub(crate) fn blast_damage(distance: f32, radius: f32, full: u32) -> u32 {
    if distance >= radius || radius <= 0.0 {
        return 0;
    }
    let portion = 1.0 - distance / radius;
    (full as f32 * portion).round() as u32
}

fn advance(
    walker: &mut Grenadier,
    mut position: Vec3,
    players: &[Vec3],
    blocks: &[Block],
    dt: f32,
) -> Advance {
    if let Gait::Reloading(timer) = walker.gait {
        let timer = timer - dt;
        walker.gait = if timer <= 0.0 {
            Gait::Walking
        } else {
            Gait::Reloading(timer)
        };
    }
    let nearest = nearest_player(position, players);
    if matches!(walker.gait, Gait::Walking)
        && nearest.is_some_and(|player| in_reach(position, player))
    {
        let weapon = launcher();
        walker.gait = Gait::Aiming(weapon.aim.as_secs_f32());
    }
    if let Gait::Aiming(timer) = walker.gait {
        let face = nearest
            .map(|player| flat_dir(position, player))
            .unwrap_or(walker.heading);
        let timer = timer - dt;
        if timer <= 0.0 {
            let weapon = launcher();
            let aim = nearest.unwrap_or_else(|| {
                position
                    + Vec3::new(walker.heading.x, 0.0, walker.heading.y) * (weapon.engage * 0.65)
            });
            let spots = dispersion(aim, aim - position, &mut walker.rng);
            walker.gait =
                Gait::Reloading((weapon.reload_each * u32::from(weapon.count)).as_secs_f32());
            return Advance {
                position,
                rotation: yaw(face),
                volley: Some(spots),
            };
        }
        walker.gait = Gait::Aiming(timer);
        return Advance {
            position,
            rotation: yaw(face),
            volley: None,
        };
    }

    let speed = blueprint().speed();
    let distance = speed * dt;
    let step = Vec3::new(walker.heading.x, 0.0, walker.heading.y) * distance;
    let proposed = position + step;
    if let Some(obstacle) = blocked(proposed, walker.radius, blocks) {
        walker.heading = turn_away(walker.heading, position, obstacle);
        let turned = position + Vec3::new(walker.heading.x, 0.0, walker.heading.y) * distance;
        if blocked(turned, walker.radius, blocks).is_none() {
            position = turned;
        }
    } else {
        position = proposed;
    }
    position.x = route::clamp_actor_x(position.x, position.z, walker.radius);
    position.y = blueprint().height();
    Advance {
        position,
        rotation: yaw(walker.heading),
        volley: None,
    }
}

fn dispersion(aim: Vec3, forward: Vec3, rng: &mut u32) -> [Vec3; 4] {
    let weapon = launcher();
    let forward = flat_norm(forward);
    let side = Vec3::new(-forward.z, 0.0, forward.x);
    let mut offsets = [Vec3::ZERO; 4];
    let spin = next_unit(rng) * std::f32::consts::TAU;
    for (index, offset) in offsets.iter_mut().enumerate() {
        let jitter = (next_unit(rng) - 0.5) * 0.7;
        let angle = spin + index as f32 * std::f32::consts::FRAC_PI_2 + jitter;
        let radius = weapon.spread_min + next_unit(rng) * (weapon.spread_max - weapon.spread_min);
        *offset = (forward * angle.cos() + side * angle.sin()) * radius;
    }
    for _ in 0..12 {
        separate_pairs(&mut offsets);
        let mean = offsets.iter().copied().sum::<Vec3>() / 4.0;
        for offset in &mut offsets {
            *offset -= mean;
        }
    }
    let mut spots = [Vec3::ZERO; 4];
    for (spot, offset) in spots.iter_mut().zip(offsets) {
        *spot = Vec3::new(aim.x + offset.x, 0.0, aim.z + offset.z);
    }
    spots
}

fn separate_pairs(offsets: &mut [Vec3; 4]) {
    for i in 0..4 {
        for j in (i + 1)..4 {
            let delta = offsets[i] - offsets[j];
            let distance = delta.length();
            let separation = launcher().separation;
            if distance >= separation {
                continue;
            }
            let push = if distance > 1.0e-4 {
                delta / distance
            } else {
                Vec3::X
            };
            let gap = (separation - distance) * 0.5;
            offsets[i] += push * gap;
            offsets[j] -= push * gap;
        }
    }
}

fn nearest_player(position: Vec3, players: &[Vec3]) -> Option<Vec3> {
    players.iter().copied().min_by(|left, right| {
        flat_distance_sq(position, *left).total_cmp(&flat_distance_sq(position, *right))
    })
}

fn in_reach(position: Vec3, player: Vec3) -> bool {
    let reach = launcher().engage;
    flat_distance_sq(position, player) <= reach * reach
}

fn flat_distance_sq(from: Vec3, to: Vec3) -> f32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    dx * dx + dz * dz
}

fn flat_dir(from: Vec3, to: Vec3) -> Vec2 {
    let dir = Vec2::new(to.x - from.x, to.z - from.z);
    if dir.length_squared() > 1.0e-4 {
        dir.normalize()
    } else {
        Vec2::Y
    }
}

fn flat_norm(forward: Vec3) -> Vec3 {
    let flat = Vec3::new(forward.x, 0.0, forward.z);
    if flat.length_squared() > 1.0e-4 {
        flat.normalize()
    } else {
        Vec3::Z
    }
}

fn yaw(heading: Vec2) -> Quat {
    Quat::from_rotation_y(heading.x.atan2(heading.y))
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

fn blocked(position: Vec3, radius: f32, blocks: &[Block]) -> Option<Vec3> {
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

fn overlaps(position: Vec3, radius: f32, block: &Block) -> bool {
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

fn cover_blocks(covers: &Query<(&Transform, &Cover), Without<Grenadier>>) -> Vec<Block> {
    covers
        .iter()
        .map(|(transform, cover)| Block {
            center: transform.translation,
            half_width: cover.half_width,
            half_depth: cover.half_depth,
        })
        .collect()
}

fn next_unit(state: &mut u32) -> f32 {
    *state ^= state.wrapping_shl(13);
    *state ^= *state >> 17;
    *state ^= state.wrapping_shl(5);
    if *state == 0 {
        *state = 0x6d2b_79f5;
    }
    *state as f32 / u32::MAX as f32
}

fn spawn_throw(
    commands: &mut Commands,
    visuals: &Visuals,
    owner: Entity,
    origin: Vec3,
    spot: Vec3,
) {
    let mark = commands
        .spawn((
            VolleyPiece,
            BlastMark { spin: 0.0 },
            Transform::from_xyz(spot.x, 0.08, spot.z),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    commands.spawn((
        Mesh3d(visuals.blast_ring_mesh.clone()),
        MeshMaterial3d(visuals.blast_mark_material.clone()),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(launcher().blast_radius)),
        ChildOf(mark),
    ));
    for turn in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
        commands.spawn((
            Mesh3d(visuals.enemy_part_mesh.clone()),
            MeshMaterial3d(visuals.blast_mark_material.clone()),
            Transform::from_rotation(Quat::from_rotation_y(turn))
                .with_scale(Vec3::new(1.6, 0.05, 0.16)),
            ChildOf(mark),
        ));
    }
    let lob = commands
        .spawn((
            VolleyPiece,
            Lob {
                origin,
                target: spot,
                age: 0.0,
                owner,
                mark,
            },
            Transform::from_translation(origin),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    commands.spawn((
        Mesh3d(visuals.grenade_mesh.clone()),
        MeshMaterial3d(visuals.grenade_material.clone()),
        Transform::default(),
        ChildOf(lob),
    ));
}

fn spawn_visual(commands: &mut Commands, visuals: &Visuals, actor: Entity, size: f32) {
    let iron = &visuals.enemy_iron_material;
    let brass = &visuals.enemy_brass_material;
    let hull = &visuals.heavy_material;
    let mut part = |material: &Handle<StandardMaterial>, position: Vec3, scale: Vec3| {
        commands.spawn((
            Mesh3d(visuals.enemy_part_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position * size).with_scale(scale * size),
            ChildOf(actor),
        ));
    };
    // Wide feet sit out to the sides so the silhouette reads as a mech.
    for side in [-1.0, 1.0] {
        part(
            iron,
            Vec3::new(side * 0.78, -0.48, 0.06),
            Vec3::new(0.62, 0.22, 0.78),
        );
        part(
            iron,
            Vec3::new(side * 0.68, -0.16, 0.0),
            Vec3::new(0.18, 0.52, 0.24),
        );
        part(
            brass,
            Vec3::new(side * 0.46, 0.08, -0.04),
            Vec3::new(0.16, 0.28, 0.36),
        );
    }
    part(hull, Vec3::new(0.0, 0.14, 0.0), Vec3::new(0.72, 0.46, 0.78));
    part(iron, Vec3::new(0.0, 0.34, 0.0), Vec3::new(1.15, 0.16, 0.42));
    part(
        brass,
        Vec3::new(0.0, 0.46, 0.42),
        Vec3::new(0.36, 0.2, 0.48),
    );
    for tube in [-0.18_f32, -0.06, 0.06, 0.18] {
        part(
            iron,
            Vec3::new(tube, 0.52, 0.72),
            Vec3::new(0.08, 0.08, 0.28),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    use crate::audio::SoundBank;

    fn sample() -> Grenadier {
        Grenadier {
            radius: blueprint().radius(),
            heading: Vec2::new(0.0, 1.0),
            gait: Gait::Walking,
            rng: 0x51ed_19ab,
        }
    }

    #[test]
    fn a_blast_fades_from_the_center_to_the_rim() {
        assert_eq!(blast_damage(0.0, 2.0, 80), 80);
        assert_eq!(blast_damage(1.0, 2.0, 80), 40);
        assert_eq!(blast_damage(2.0, 2.0, 80), 0);
        assert_eq!(blast_damage(3.0, 2.0, 80), 0);
    }

    #[test]
    fn the_volley_stays_centered_sparse_and_can_pass_the_aim() {
        let aim = Vec3::new(0.0, 0.0, 10.0);
        let mut rng = 0x51ed_19ab;
        let spots = dispersion(aim, Vec3::Z, &mut rng);
        let mean = spots.iter().copied().sum::<Vec3>() / 4.0;
        assert!((mean.x - aim.x).abs() < 0.02, "center x {}", mean.x);
        assert!((mean.z - aim.z).abs() < 0.02, "center z {}", mean.z);
        for (index, spot) in spots.iter().enumerate() {
            for other in spots.iter().skip(index + 1) {
                let gap = Vec2::new(spot.x - other.x, spot.z - other.z).length();
                assert!(
                    gap + 0.05 >= launcher().separation,
                    "grenades landed {gap} apart"
                );
            }
        }
        let furthest = spots
            .iter()
            .map(|spot| spot.length())
            .fold(0.0_f32, f32::max);
        assert!(
            furthest > aim.length() + 0.3,
            "the pattern should pass the aim point, furthest {furthest}"
        );
    }

    #[test]
    fn meeting_cover_turns_the_walker_ninety_degrees() {
        let mut walker = sample();
        let before = walker.heading;
        let position = Vec3::new(0.0, blueprint().height(), 0.0);
        let blocks = [Block {
            center: Vec3::new(0.0, 0.55, 3.0),
            half_width: 2.7,
            half_depth: 0.5,
        }];
        let step = advance(&mut walker, position, &[], &blocks, 1.0);
        let dot = before.dot(walker.heading);
        assert!(
            dot.abs() < 0.15,
            "a meeting should turn about ninety degrees, dot {dot}"
        );
        assert!(
            blocked(step.position, walker.radius, &blocks).is_none(),
            "the walker should be clear of the cover at {:?}",
            step.position
        );
    }

    #[test]
    fn the_walker_holds_still_for_the_aim_then_throws_and_walks_the_reload() {
        let mut walker = sample();
        let players = [Vec3::new(2.0, 0.8, 8.0)];
        let position = Vec3::new(0.0, blueprint().height(), 0.0);
        let held = advance(&mut walker, position, &players, &[], 0.5);
        assert!(held.volley.is_none());
        assert_eq!(held.position, position);
        assert!(matches!(walker.gait, Gait::Aiming(timer) if (timer - 1.5).abs() < 0.001));
        let thrown = advance(&mut walker, held.position, &players, &[], 1.5);
        let spots = thrown.volley.expect("volley");
        assert_eq!(spots.len(), 4);
        let reload = match walker.gait {
            Gait::Reloading(timer) => timer,
            other => panic!("expected reload, got {other:?}"),
        };
        assert!((reload - 4.0).abs() < 0.001);
        let walked = advance(&mut walker, thrown.position, &players, &[], 0.5);
        assert!(walked.volley.is_none());
        assert!(
            walked.position.distance(thrown.position) > 0.2,
            "reload should leave the walker moving"
        );
    }

    #[test]
    fn a_landing_grenade_stings_by_distance() {
        let mut app = App::new();
        app.add_message::<Struck>()
            .insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, fly_grenades);
        let target = Vec3::new(4.0, 0.0, -6.0);
        let owner = app.world_mut().spawn_empty().id();
        let mark = app
            .world_mut()
            .spawn((BlastMark { spin: 0.0 }, Transform::from_translation(target)))
            .id();
        app.world_mut().spawn((
            Player,
            Health(1000),
            Transform::from_translation(target + Vec3::Y),
        ));
        app.world_mut().spawn((
            sample(),
            Health(100),
            Transform::from_xyz(target.x + 1.0, 0.95, target.z),
        ));
        app.world_mut().spawn((
            Lob {
                origin: target + Vec3::new(0.0, 1.0, -8.0),
                target,
                age: launcher().flight.as_secs_f32() - 0.01,
                owner,
                mark,
            },
            Transform::from_translation(target),
        ));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
        let mut damages: Vec<_> = app
            .world()
            .resource::<Messages<Struck>>()
            .iter_current_update_messages()
            .map(|strike| strike.damage)
            .collect();
        damages.sort_unstable();
        assert_eq!(damages, vec![40, 80]);
    }

    #[test]
    fn overlapping_landings_list_the_strongest_sting_first() {
        let mut app = App::new();
        app.add_message::<Struck>()
            .insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, fly_grenades);
        let player = app
            .world_mut()
            .spawn((Player, Health(1000), Transform::from_xyz(0.0, 0.75, 0.0)))
            .id();
        let owner = app.world_mut().spawn_empty().id();
        for (target, age) in [
            (
                Vec3::new(1.0, 0.0, 0.0),
                launcher().flight.as_secs_f32() - 0.01,
            ),
            (Vec3::ZERO, launcher().flight.as_secs_f32() - 0.01),
        ] {
            let mark = app
                .world_mut()
                .spawn((BlastMark { spin: 0.0 }, Transform::from_translation(target)))
                .id();
            app.world_mut().spawn((
                Lob {
                    origin: target + Vec3::Y,
                    target,
                    age,
                    owner,
                    mark,
                },
                Transform::from_translation(target),
            ));
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
        let damages: Vec<_> = app
            .world()
            .resource::<Messages<Struck>>()
            .iter_current_update_messages()
            .filter(|strike| strike.target == player)
            .map(|strike| strike.damage)
            .collect();
        assert_eq!(damages, vec![80, 40]);
    }
}
