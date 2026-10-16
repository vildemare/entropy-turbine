//! A wide-footed walker that lobs a sparse volley of grenades.
//!
//! The step itself lives with the other locomotion. This launcher owns the
//! aim and the throw. A player inside its reach stops the walker: it turns
//! toward that player, aims for two seconds, then throws four grenades around
//! the aim point. The aim point is the center of the pattern. Reload is one
//! second per grenade, and the walker moves during that reload. Each landing
//! is a circle on the ground; damage falls off from the center to the rim.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::behavior::{WalkerHeading, WalkerHold};
use super::{EnemyKind, EnemyReward};
use crate::ability::Struck;
use crate::audio::{self, SoundBank};
use crate::character::{CharacterDefinition, LauncherSpec};
use crate::combat::{BlastRecovery, Delivery, EnergyShield, Faction, Health, HitCircle, Material};
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::visuals::Visuals;

fn blueprint() -> &'static CharacterDefinition {
    crate::character::grenadier()
}

fn launcher() -> &'static LauncherSpec {
    blueprint().launcher()
}

#[derive(Component)]
pub struct Grenadier {
    pub radius: f32,
    gait: Gait,
    rng: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gait {
    Walking,
    Aiming(f32),
    Reloading(f32),
}

struct GaitTick {
    hold: bool,
    face: Vec2,
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
                gait: Gait::Walking,
                rng: rng | 1,
            },
            WalkerHeading(heading),
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

pub fn aim_grenadiers(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<Grenadier>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut holds: MessageWriter<WalkerHold>,
    mut walkers: Query<
        (Entity, &Transform, &mut Grenadier, &WalkerHeading),
        (Without<Player>, Without<BlastRecovery>),
    >,
    players: Query<&Transform, (With<Player>, Without<Grenadier>)>,
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
    let player_positions: Vec<Vec3> = players
        .iter()
        .map(|transform| transform.translation)
        .collect();
    let dt = time.delta_secs();

    for (entity, transform, mut walker, heading) in &mut walkers {
        if transform.translation.z > exit_z {
            commands.entity(entity).despawn();
            continue;
        }
        let tick = tick_gait(
            &mut walker,
            transform.translation,
            heading.0,
            &player_positions,
            dt,
        );
        if tick.hold {
            holds.write(WalkerHold {
                walker: entity,
                face: tick.face,
            });
        }
        if let Some(spots) = tick.volley {
            audio::play_game(&mut commands, &sounds.grenade_lob, 0.28, 1.0);
            let origin = Vec3::new(
                transform.translation.x,
                transform.translation.y + 0.35,
                transform.translation.z,
            );
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

fn tick_gait(
    walker: &mut Grenadier,
    position: Vec3,
    heading: Vec2,
    players: &[Vec3],
    dt: f32,
) -> GaitTick {
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
            .unwrap_or(heading);
        let timer = timer - dt;
        if timer <= 0.0 {
            let weapon = launcher();
            let aim = nearest
                .map(|player| aim_within_reach(position, player))
                .unwrap_or_else(|| {
                    position + Vec3::new(heading.x, 0.0, heading.y) * (weapon.engage * 0.65)
                });
            let spots = dispersion(aim, aim - position, &mut walker.rng);
            walker.gait =
                Gait::Reloading((weapon.reload_each * u32::from(weapon.count)).as_secs_f32());
            return GaitTick {
                hold: true,
                face,
                volley: Some(spots),
            };
        }
        walker.gait = Gait::Aiming(timer);
        return GaitTick {
            hold: true,
            face,
            volley: None,
        };
    }
    GaitTick {
        hold: false,
        face: heading,
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

fn aim_within_reach(position: Vec3, player: Vec3) -> Vec3 {
    let offset = Vec3::new(player.x - position.x, 0.0, player.z - position.z);
    let reach = launcher().engage;
    if offset.length_squared() <= reach * reach {
        return player;
    }
    let aim = position + offset.normalize() * reach;
    Vec3::new(aim.x, player.y, aim.z)
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

    use super::super::behavior::{self, Footprint};
    use super::*;

    use crate::audio::SoundBank;

    struct Advance {
        position: Vec3,
        volley: Option<[Vec3; 4]>,
    }

    fn sample() -> Grenadier {
        Grenadier {
            radius: blueprint().radius(),
            gait: Gait::Walking,
            rng: 0x51ed_19ab,
        }
    }

    /// Aim first, then the shared step, in the same order as the frame.
    fn advance(
        walker: &mut Grenadier,
        heading: &mut Vec2,
        position: Vec3,
        players: &[Vec3],
        blocks: &[Footprint],
        dt: f32,
    ) -> Advance {
        let tick = tick_gait(walker, position, *heading, players, dt);
        if tick.hold {
            return Advance {
                position,
                volley: tick.volley,
            };
        }
        let (position, next) = behavior::step_walker(
            position,
            *heading,
            walker.radius,
            blueprint().speed() * dt,
            blueprint().height(),
            blocks,
        );
        *heading = next;
        Advance {
            position,
            volley: None,
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
        let mut heading = Vec2::new(0.0, 1.0);
        let before = heading;
        let position = Vec3::new(0.0, blueprint().height(), 0.0);
        let blocks = [Footprint {
            center: Vec3::new(0.0, 0.55, 3.0),
            half_width: 2.7,
            half_depth: 0.5,
        }];
        let step = advance(&mut walker, &mut heading, position, &[], &blocks, 1.0);
        let dot = before.dot(heading);
        assert!(
            dot.abs() < 0.15,
            "a meeting should turn about ninety degrees, dot {dot}"
        );
        assert!(
            behavior::walker_blocked(step.position, walker.radius, &blocks).is_none(),
            "the walker should be clear of the cover at {:?}",
            step.position
        );
    }

    #[test]
    fn the_walker_holds_still_for_the_aim_then_throws_and_walks_the_reload() {
        let mut walker = sample();
        let mut heading = Vec2::new(0.0, 1.0);
        let players = [Vec3::new(2.0, 0.8, 8.0)];
        let position = Vec3::new(0.0, blueprint().height(), 0.0);
        let held = advance(&mut walker, &mut heading, position, &players, &[], 0.5);
        assert!(held.volley.is_none());
        assert_eq!(held.position, position);
        assert!(matches!(walker.gait, Gait::Aiming(timer) if (timer - 1.5).abs() < 0.001));
        let thrown = advance(&mut walker, &mut heading, held.position, &players, &[], 1.5);
        let spots = thrown.volley.expect("volley");
        assert_eq!(spots.len(), 4);
        let reload = match walker.gait {
            Gait::Reloading(timer) => timer,
            other => panic!("expected reload, got {other:?}"),
        };
        assert!((reload - 4.0).abs() < 0.001);
        let walked = advance(
            &mut walker,
            &mut heading,
            thrown.position,
            &players,
            &[],
            0.5,
        );
        assert!(walked.volley.is_none());
        assert!(
            walked.position.distance(thrown.position) > 0.2,
            "reload should leave the walker moving"
        );
    }

    #[test]
    fn a_locked_player_cannot_pull_the_volley_beyond_the_engagement_range() {
        let mut walker = sample();
        let mut heading = Vec2::Y;
        let position = Vec3::new(0.0, blueprint().height(), 0.0);
        let locked = [Vec3::new(0.0, 0.8, launcher().engage - 1.0)];
        let held = advance(&mut walker, &mut heading, position, &locked, &[], 0.5);
        assert!(held.volley.is_none());

        let escaped = [Vec3::new(0.0, 0.8, launcher().engage + 30.0)];
        let thrown = advance(
            &mut walker,
            &mut heading,
            held.position,
            &escaped,
            &[],
            launcher().aim.as_secs_f32() - 0.5,
        );
        let spots = thrown.volley.expect("volley");
        let center = spots.iter().copied().sum::<Vec3>() / spots.len() as f32;
        let range = Vec2::new(center.x - position.x, center.z - position.z).length();
        assert!(
            (range - launcher().engage).abs() < 0.02,
            "volley center was {range} metres away"
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
