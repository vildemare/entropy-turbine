//! Slow tracked machines that cross the field and throw a charged jolt.
//!
//! They are not the gunner squads and not the Entropy Turbine bosses. A cow
//! looks ahead along its lane and commits to a clear route around the whole
//! cover footprint instead of steering back into it. The prongs stay dark
//! until a player enters the jolt's reach, and that entry starts the whine.
//! Local +Z is the direction of travel. A finished charge fires toward the
//! nearest player, or along local +Z when the lane is empty.

use std::time::Duration;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::behavior::{self, Drift};
use super::{Cover, EnemyKind, EnemyReward};
use crate::audio::{self, SoundBank};
use crate::character::{CharacterDefinition, ZapperSpec};
use crate::combat::{
    BlastRecovery, Damage, Delivery, Faction, Health, HitCircle, Lifetime, Material, Projectile,
};
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::route;
use crate::world::visuals::{self, Visuals};

fn cow() -> &'static CharacterDefinition {
    crate::character::zap_bot()
}

fn jolt() -> &'static ZapperSpec {
    cow().zapper()
}

/// When cows enter the field. This is the encounter schedule, not the jolt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZapSchedule {
    pub delay: Duration,
    pub interval: Duration,
    pub max_alive: usize,
}

pub(crate) const ZAP_SCHEDULE: ZapSchedule = ZapSchedule {
    delay: Duration::from_millis(5_500),
    interval: Duration::from_millis(8_000),
    max_alive: 3,
};

#[derive(Component)]
pub struct ZapBot {
    pub radius: f32,
    lane: f32,
    side: f32,
    avoidance: Option<behavior::TrackAvoidance>,
    drift: Drift,
    charge: f32,
    rest: f32,
    flash: f32,
    voiced: bool,
    prongs: Handle<StandardMaterial>,
}

struct ChargeTick {
    start_whine: bool,
    discharge: bool,
}

/// The drive saw a player, or the lane ahead. The jolt spends its own charge.
#[derive(Message, Clone, Copy, Debug)]
pub struct JoltCue {
    pub bot: Entity,
    pub in_range: bool,
    pub aim: Vec3,
    pub origin: Vec3,
}

pub fn spawn_zap_bots(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut director: ResMut<super::EncounterDirector>,
    mut commands: Commands,
    cameras: Query<&Transform, With<FollowCamera>>,
    covers: Query<(&Transform, &Cover), Without<ZapBot>>,
    alive: Query<(), With<ZapBot>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    director.zap_timer -= time.delta_secs();
    if director.zap_timer > 0.0 {
        return;
    }
    director.zap_timer = ZAP_SCHEDULE.interval.as_secs_f32();
    if alive.iter().count() >= ZAP_SCHEDULE.max_alive {
        return;
    }

    let hull = cow();
    let center_z = camera::framed_ground_z(camera);
    let z = center_z - 32.0;
    let lane = director.random() * 1.6 - 0.8;
    let side = if director.random() < 0.5 { -1.0 } else { 1.0 };
    let mut x = route::clamp_actor_x(route::centerline_x(z) + lane * 8.0, z, hull.radius());
    let blocks = behavior::footprints(covers.iter());
    for _ in 0..5 {
        if !behavior::tracks_blocked(Vec3::new(x, hull.height(), z), hull.radius(), &blocks) {
            break;
        }
        x = route::clamp_actor_x(x + side * 2.4, z, hull.radius());
    }

    let prongs = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.38, 0.16),
        emissive: LinearRgba::BLACK,
        ..default()
    });
    let actor = commands
        .spawn((
            ZapBot {
                radius: hull.radius(),
                lane,
                side,
                avoidance: None,
                drift: Drift::default(),
                charge: 0.0,
                rest: 0.0,
                flash: 0.0,
                voiced: false,
                prongs: prongs.clone(),
            },
            EnemyReward {
                kind: EnemyKind::ZapBot,
                hit_player: false,
            },
            Health(hull.health()),
            hull.runtime_abilities(),
            HitCircle {
                radius: hull.radius(),
                faction: Faction::Enemy,
                swell: true,
            },
            Transform::from_xyz(x, hull.height(), z),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    spawn_visual(&mut commands, &visuals, actor, hull.visual_scale(), prongs);
}

pub fn drive_zap_bots(
    time: Res<Time>,
    session: Res<Session>,
    mut commands: Commands,
    mut cues: MessageWriter<JoltCue>,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<ZapBot>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    covers: Query<(&Transform, &Cover), Without<ZapBot>>,
    mut bots: Query<
        (Entity, &mut Transform, &mut ZapBot),
        (Without<Player>, Without<BlastRecovery>),
    >,
    players: Query<&Transform, (With<Player>, Without<ZapBot>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
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
    let blocks = behavior::footprints(covers.iter());
    let player_positions: Vec<Vec3> = players
        .iter()
        .map(|transform| transform.translation)
        .collect();
    let speed = cow().speed();

    for (entity, mut transform, mut bot) in &mut bots {
        if transform.translation.z > exit_z {
            commands.entity(entity).despawn();
            continue;
        }
        let position = transform.translation;
        let lane_offset = bot.lane * 7.0;
        let max_step = speed * dt;
        let radius = bot.radius;
        let side = bot.side;
        let back_step = behavior::reverse_step(&bot.drift, side, max_step);
        let (mut next, chosen_side) = if let Some(back) = back_step {
            // A route calculated while reversing would already be stale when
            // the backup ends. Replan from the position actually reached.
            bot.avoidance = None;
            (position + back, side)
        } else {
            behavior::step_tracks(
                position,
                radius,
                lane_offset,
                side,
                max_step,
                &blocks,
                &mut bot.avoidance,
            )
        };
        // A cow keeps driving through the backup. It does not stand to shoot.
        bot.drift.from = position;
        bot.drift.fresh = true;
        bot.drift.trying = true;
        bot.drift.step = Vec2::new(next.x - position.x, next.z - position.z);
        next.x = route::clamp_actor_x(next.x, next.z, bot.radius);
        next.y = position.y;
        for (cover_transform, cover) in &covers {
            super::separate_from_cover(&mut next, bot.radius, cover_transform.translation, cover);
        }
        next.x = route::clamp_actor_x(next.x, next.z, bot.radius);
        let moved = Vec2::new(next.x - position.x, next.z - position.z);
        let started = behavior::note_drift(&mut bot.drift, moved, dt);
        if started {
            bot.side = -bot.side;
            bot.avoidance = None;
        } else if bot.drift.backing <= 0.0 && bot.drift.stalled == 0.0 {
            bot.side = chosen_side;
        }
        let travel = next - position;
        if travel.length_squared() > 0.0001 {
            transform.rotation = Quat::from_rotation_y(travel.x.atan2(travel.z));
        }
        transform.translation = next;

        // Local +Z is travel. Bevy's forward() is local −Z, so the facing used
        // for an empty lane is the same vector the hull is built along.
        let facing = transform.rotation * Vec3::Z;
        let in_range = player_in_bolt_range(transform.translation, &player_positions);
        let aim = aim_direction(transform.translation, facing, &player_positions);
        let origin = bolt_origin(transform.translation, aim, bot.radius);
        cues.write(JoltCue {
            bot: entity,
            in_range,
            aim,
            origin,
        });
    }
}

pub fn charge_jolts(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut cues: MessageReader<JoltCue>,
    mut bots: Query<&mut ZapBot>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
    for cue in cues.read().copied() {
        let Ok(mut bot) = bots.get_mut(cue.bot) else {
            continue;
        };
        // The whine is the charge. It starts when a player is inside the
        // jolt's reach, and a rise already underway still finishes.
        let tick = step_charge(&mut bot, dt, cue.in_range);
        if tick.start_whine {
            audio::play_game(&mut commands, &sounds.zap_charge, 0.26, 1.0);
        }
        if tick.discharge {
            spawn_bolt(&mut commands, &visuals, cue.bot, cue.origin, cue.aim);
            audio::play_game(&mut commands, &sounds.zap, 0.36, 1.0);
        }
        if let Some(mut material) = materials.get_mut(&bot.prongs) {
            let heat = (bot.charge / jolt().charge.as_secs_f32()).clamp(0.0, 1.0);
            let glow = heat.max(bot.flash);
            material.base_color =
                Color::srgb(0.55 + 0.45 * glow, 0.26 + 0.58 * glow, 0.10 + 0.55 * glow);
            material.emissive =
                LinearRgba::new(1.4 + 5.0 * glow, 0.28 + 1.5 * glow, 0.04 + 0.3 * glow, 1.0);
        }
    }
}

fn player_in_bolt_range(position: Vec3, players: &[Vec3]) -> bool {
    let reach = jolt().reach;
    players.iter().any(|player| {
        let dx = player.x - position.x;
        let dz = player.z - position.z;
        dx * dx + dz * dz <= reach * reach
    })
}

fn step_charge(bot: &mut ZapBot, mut dt: f32, in_range: bool) -> ChargeTick {
    bot.flash = (bot.flash - dt * 5.0).max(0.0);
    if bot.rest > 0.0 {
        if bot.rest > dt {
            bot.rest -= dt;
            return ChargeTick {
                start_whine: false,
                discharge: false,
            };
        }
        dt -= bot.rest;
        bot.rest = 0.0;
    }
    if !bot.voiced {
        // Quiet until a player is close enough for the jolt to reach them.
        // A whine that has already started keeps rising below.
        if !in_range {
            return ChargeTick {
                start_whine: false,
                discharge: false,
            };
        }
        bot.voiced = true;
        bot.charge = dt;
        if bot.charge >= jolt().charge.as_secs_f32() {
            let mut tick = finish_charge(bot);
            tick.start_whine = true;
            return tick;
        }
        return ChargeTick {
            start_whine: true,
            discharge: false,
        };
    }
    bot.charge += dt;
    if bot.charge >= jolt().charge.as_secs_f32() {
        return finish_charge(bot);
    }
    ChargeTick {
        start_whine: false,
        discharge: false,
    }
}

fn finish_charge(bot: &mut ZapBot) -> ChargeTick {
    bot.charge = 0.0;
    bot.voiced = false;
    bot.rest = jolt().rest.as_secs_f32();
    bot.flash = 1.0;
    ChargeTick {
        start_whine: false,
        discharge: true,
    }
}

fn aim_direction(position: Vec3, facing: Vec3, players: &[Vec3]) -> Vec3 {
    let toward_player = players
        .iter()
        .min_by(|left, right| {
            left.distance_squared(position)
                .total_cmp(&right.distance_squared(position))
        })
        .map(|player| Vec3::new(player.x - position.x, 0.0, player.z - position.z))
        .filter(|offset| offset.length_squared() > 1.0e-4);
    let flat = toward_player.unwrap_or_else(|| Vec3::new(facing.x, 0.0, facing.z));
    let flat = Vec3::new(flat.x, 0.0, flat.z);
    if flat.length_squared() > 1.0e-4 {
        flat.normalize()
    } else {
        Vec3::Z
    }
}

fn bolt_origin(position: Vec3, aim: Vec3, radius: f32) -> Vec3 {
    position + Vec3::new(aim.x, 0.0, aim.z) * (radius + 0.45) + Vec3::Y * 0.2
}

fn spawn_bolt(commands: &mut Commands, visuals: &Visuals, owner: Entity, origin: Vec3, aim: Vec3) {
    let weapon = jolt();
    let speed = weapon.bolt_speed;
    let bolt = commands
        .spawn((
            Projectile {
                direction: aim,
                speed,
                radius: weapon.bolt_radius,
                previous: origin,
                spent: false,
                faction: Faction::Enemy,
                material: Material::Electricity,
                delivery: Delivery::Energy,
                owner,
            },
            Damage(weapon.damage),
            Lifetime(weapon.reach / speed),
            Transform::from_translation(origin)
                .with_rotation(Quat::from_rotation_y(aim.x.atan2(aim.z))),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    // The cuboid is centered, so shift it forward by half its length. Local +Z
    // is the shot direction, which puts the tail on the prongs.
    commands.spawn((
        Mesh3d(visuals.zap_bolt_mesh.clone()),
        MeshMaterial3d(visuals.zap_bolt_material.clone()),
        Transform::from_translation(Vec3::Z * (visuals::ZAP_BOLT_LENGTH * 0.5)),
        ChildOf(bolt),
    ));
}

fn spawn_visual(
    commands: &mut Commands,
    visuals: &Visuals,
    actor: Entity,
    size: f32,
    prongs: Handle<StandardMaterial>,
) {
    let iron = &visuals.enemy_iron_material;
    let brass = &visuals.enemy_brass_material;
    let body = &visuals.heavy_material;
    let mut part = |material: &Handle<StandardMaterial>, position: Vec3, scale: Vec3| {
        commands.spawn((
            Mesh3d(visuals.enemy_part_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position * size).with_scale(scale * size),
            ChildOf(actor),
        ));
    };
    for side in [-1.0, 1.0] {
        part(
            iron,
            Vec3::new(side * 0.52, -0.16, 0.0),
            Vec3::new(0.28, 0.22, 1.35),
        );
        part(
            &prongs,
            Vec3::new(side * 0.16, 0.18, 0.62),
            Vec3::new(0.08, 0.2, 0.28),
        );
    }
    part(body, Vec3::ZERO, Vec3::new(0.82, 0.36, 1.05));
    part(
        brass,
        Vec3::new(0.0, 0.28, -0.18),
        Vec3::new(0.46, 0.26, 0.52),
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::behavior::{self, Footprint};
    use super::*;

    fn barricade(z: f32) -> Footprint {
        Footprint {
            center: Vec3::new(0.0, 0.0, z),
            half_width: 2.7,
            half_depth: 0.5,
        }
    }

    #[test]
    fn an_open_lane_carries_a_cow_downscreen() {
        let mut avoidance = None;
        let (next, side) =
            behavior::step_tracks(Vec3::ZERO, 0.9, 0.0, 1.0, 0.4, &[], &mut avoidance);
        assert!(next.z > 0.2);
        assert_eq!(side, 1.0);
    }

    #[test]
    fn a_barricade_ahead_pushes_the_cow_to_one_side() {
        let blocks = [barricade(1.2)];
        let start = Vec3::new(0.0, 0.0, -0.5);
        let mut avoidance = None;
        let (next, side) =
            behavior::step_tracks(start, 0.9, 0.0, 1.0, 0.4, &blocks, &mut avoidance);
        assert!(next.x.abs() > 0.05, "x {}", next.x);
        assert!(next.z > start.z);
        assert_eq!(side.abs(), 1.0);
        assert!(!behavior::tracks_blocked(next, 0.9, &blocks));
    }

    #[test]
    fn a_cow_turns_out_several_metres_before_the_barricade() {
        let start = Vec3::new(0.0, 0.0, -6.0);
        let blocks = [barricade(0.0)];
        let mut open_avoidance = None;
        let (straight, _) =
            behavior::step_tracks(start, 0.9, 0.0, 1.0, 0.5, &[], &mut open_avoidance);
        let mut avoidance = None;
        let (next, side) =
            behavior::step_tracks(start, 0.9, 0.0, 1.0, 0.5, &blocks, &mut avoidance);
        assert!(next.x.abs() > straight.x.abs() + 0.08, "x {}", next.x);
        assert!(next.z > start.z);
        assert_eq!(side.abs(), 1.0);
    }

    #[test]
    fn a_cow_commits_to_the_detour_until_the_barricade_is_behind_it() {
        let blocks = [barricade(8.0)];
        let mut position = Vec3::ZERO;
        let mut avoidance = None;
        let mut side = 1.0;
        let mut passed_face = false;
        let mut detoured = false;

        for _ in 0..180 {
            let (next, chosen) =
                behavior::step_tracks(position, 0.9, 0.0, side, 0.15, &blocks, &mut avoidance);
            assert!(
                !behavior::tracks_blocked(next, 0.9, &blocks),
                "cow entered the barricade at {next}"
            );
            if (7.5..=8.5).contains(&next.z) {
                passed_face = true;
                detoured |= next.x.abs() > 3.6;
            }
            position = next;
            side = chosen;
        }

        assert!(passed_face, "cow never passed the barricade");
        assert!(detoured, "cow did not keep clear while alongside it");
        assert!(position.z > 20.0, "cow stopped at z {}", position.z);
        assert!(avoidance.is_none(), "detour stayed active after the pass");
        assert!(
            position.x.abs() < 3.0,
            "cow did not return to its lane: {position}"
        );
    }

    #[test]
    fn a_cow_plans_around_the_whole_cover_cluster() {
        let blocks = [
            barricade(8.0),
            Footprint {
                center: Vec3::new(4.8, 0.0, 9.0),
                half_width: 1.1,
                half_depth: 1.0,
            },
            Footprint {
                center: Vec3::new(-5.4, 0.0, 10.0),
                half_width: 0.8,
                half_depth: 0.8,
            },
        ];
        let mut position = Vec3::ZERO;
        let mut avoidance = None;
        let mut side = 1.0;
        let mut planned = false;

        for _ in 0..220 {
            let (next, chosen) =
                behavior::step_tracks(position, 0.9, 0.0, side, 0.15, &blocks, &mut avoidance);
            planned |= avoidance.is_some();
            assert!(
                !behavior::tracks_blocked(next, 0.9, &blocks),
                "cow entered clustered cover at {next}"
            );
            position = next;
            side = chosen;
        }

        assert!(planned, "cluster never produced a committed route");
        assert!(
            position.z > 24.0,
            "cow failed to clear the cluster: {position}"
        );
        assert!(avoidance.is_none(), "cluster route never completed");
    }

    #[test]
    fn a_cow_crosses_a_complete_wave_on_a_bending_road() {
        let wave_z = -30.0;
        for wave_offset in [-5.0, 0.0, 5.0] {
            for lane_offset in [-5.5, 0.0, 5.5] {
                let blocks = [
                    Footprint {
                        center: Vec3::new(route::centerline_x(wave_z) + wave_offset, 0.0, wave_z),
                        half_width: 2.7,
                        half_depth: 0.5,
                    },
                    Footprint {
                        center: Vec3::new(
                            route::centerline_x(wave_z - 0.15) + wave_offset - 2.6,
                            0.0,
                            wave_z - 0.15,
                        ),
                        half_width: 0.65,
                        half_depth: 0.7,
                    },
                    Footprint {
                        center: Vec3::new(
                            route::centerline_x(wave_z - 0.15) + wave_offset + 2.6,
                            0.0,
                            wave_z - 0.15,
                        ),
                        half_width: 0.65,
                        half_depth: 0.7,
                    },
                    Footprint {
                        center: Vec3::new(
                            route::centerline_x(wave_z + 9.0) - 7.5,
                            0.0,
                            wave_z + 9.0,
                        ),
                        half_width: 1.05,
                        half_depth: 0.95,
                    },
                    Footprint {
                        center: Vec3::new(
                            route::centerline_x(wave_z - 11.0) + 7.5,
                            0.0,
                            wave_z - 11.0,
                        ),
                        half_width: 1.05,
                        half_depth: 0.95,
                    },
                ];
                let start_z = wave_z - 18.0;
                let mut position =
                    Vec3::new(route::centerline_x(start_z) + lane_offset, 0.0, start_z);
                let mut avoidance = None;
                let mut side = 1.0;

                for _ in 0..300 {
                    let (next, chosen) = behavior::step_tracks(
                        position,
                        0.9,
                        lane_offset,
                        side,
                        0.15,
                        &blocks,
                        &mut avoidance,
                    );
                    assert!(
                        !behavior::tracks_blocked(next, 0.9, &blocks),
                        "cow entered wave cover at {next}; wave {wave_offset}, lane {lane_offset}"
                    );
                    assert!(
                        (next.x - route::centerline_x(next.z)).abs()
                            <= route::ROAD_HALF_WIDTH - 0.9,
                        "cow left the road at {next}; wave {wave_offset}, lane {lane_offset}"
                    );
                    position = next;
                    side = chosen;
                }

                assert!(
                    position.z > wave_z + 16.0,
                    "cow did not cross the wave: {position}; wave {wave_offset}, lane {lane_offset}"
                );
                assert!(avoidance.is_none(), "wave route never completed");
            }
        }
    }

    #[test]
    fn a_charge_rises_for_one_and_a_half_seconds_then_discharges() {
        let mut bot = sample_bot();
        let first = step_charge(&mut bot, 0.05, true);
        assert!(first.start_whine);
        assert!(!first.discharge);
        let mut steps = 1;
        loop {
            steps += 1;
            let tick = step_charge(&mut bot, 0.05, true);
            assert!(!tick.start_whine);
            if tick.discharge {
                break;
            }
            assert!(steps < 40, "charge never finished");
        }
        let elapsed = steps as f32 * 0.05;
        assert!(
            (elapsed - jolt().charge.as_secs_f32()).abs() < 0.051,
            "discharged after {elapsed}s"
        );
        assert!(bot.rest > 0.0);
        assert_eq!(bot.flash, 1.0);
        let resting = step_charge(&mut bot, 0.05, true);
        assert!(!resting.start_whine);
        assert!(!resting.discharge);
    }

    #[test]
    fn the_next_whine_starts_again_after_the_rest() {
        let mut bot = sample_bot();
        bot.rest = 0.25;
        assert!(!step_charge(&mut bot, 0.125, true).start_whine);
        let tick = step_charge(&mut bot, 0.125, true);
        assert!(tick.start_whine);
        assert!(!tick.discharge);
    }

    #[test]
    fn a_discharge_aims_at_a_distant_player() {
        let players = [Vec3::new(3.0, 0.8, 12.0)];
        let aim = aim_direction(Vec3::ZERO, Vec3::Z, &players);
        let expected = Vec3::new(3.0, 0.0, 12.0).normalize();
        assert!(aim.dot(expected) > 0.999, "{aim}");
    }

    #[test]
    fn an_empty_lane_discharges_along_the_cows_facing() {
        let travel = Vec3::new(0.6, 0.0, 0.8).normalize();
        let facing = Quat::from_rotation_y(travel.x.atan2(travel.z)) * Vec3::Z;
        assert!(
            facing.dot(travel) > 0.999,
            "local +Z should follow travel, got {facing}"
        );
        let aim = aim_direction(Vec3::new(2.0, 0.46, -4.0), facing, &[]);
        assert!(aim.dot(travel) > 0.999, "{aim}");
        let origin = bolt_origin(Vec3::new(2.0, 0.46, -4.0), aim, 0.9);
        let offset = origin - Vec3::new(2.0, 0.46, -4.0);
        assert!(offset.length() > 0.9);
        assert!(offset.dot(aim) > 0.9);
    }

    #[test]
    fn a_cow_fires_toward_a_player_standing_well_away() {
        let fired = discharge_once(nearly_charged(), Some(Vec3::new(4.0, 0.8, 12.0)));
        let expected = Vec3::new(4.0 - fired.cow.x, 0.0, 12.0 - fired.cow.z).normalize();
        assert!(
            fired.direction.dot(expected) > 0.999,
            "{} vs {expected}",
            fired.direction
        );
        assert_bolt_faces_its_shot(&fired);
    }

    #[test]
    fn bolt_range_is_measured_on_the_ground() {
        let cow = Vec3::new(0.0, 0.46, 0.0);
        let reach = jolt().reach;
        assert!(player_in_bolt_range(cow, &[Vec3::new(0.0, 5.0, reach)]));
        assert!(!player_in_bolt_range(
            cow,
            &[Vec3::new(0.0, 0.8, reach + 0.05)]
        ));
    }

    #[test]
    fn an_idle_cow_stays_quiet_until_a_player_enters_bolt_range() {
        let mut bot = sample_bot();
        let quiet = step_charge(&mut bot, 0.2, false);
        assert!(!quiet.start_whine);
        assert!(!quiet.discharge);
        assert!(!bot.voiced);
        assert_eq!(bot.charge, 0.0);

        let started = step_charge(&mut bot, 0.05, true);
        assert!(started.start_whine);
        assert!(!started.discharge);
        assert!(bot.voiced);
        assert!(bot.charge > 0.0);
    }

    #[test]
    fn a_started_charge_finishes_after_the_player_steps_out() {
        let mut bot = sample_bot();
        assert!(step_charge(&mut bot, 0.05, true).start_whine);
        let mut steps = 1;
        loop {
            steps += 1;
            let tick = step_charge(&mut bot, 0.05, false);
            assert!(!tick.start_whine);
            if tick.discharge {
                break;
            }
            assert!(steps < 40, "charge never finished");
        }
        let elapsed = steps as f32 * 0.05;
        assert!(
            (elapsed - jolt().charge.as_secs_f32()).abs() < 0.051,
            "discharged after {elapsed}s"
        );
    }

    #[test]
    fn the_next_whine_waits_when_the_player_is_still_out_of_reach() {
        let mut bot = sample_bot();
        bot.rest = 0.1;
        let tick = step_charge(&mut bot, 0.2, false);
        assert!(!tick.start_whine);
        assert!(!tick.discharge);
        assert!(!bot.voiced);
        assert_eq!(bot.rest, 0.0);
        assert_eq!(bot.charge, 0.0);
    }

    #[test]
    fn a_cow_stays_quiet_while_the_player_is_outside_the_jolt() {
        let cow = drive_one_cow(Some(Vec3::new(0.0, 0.8, 12.0)), 200);
        assert!(!cow.voiced);
        assert_eq!(cow.charge, 0.0);
        assert_eq!(cow.bolts, 0);
    }

    #[test]
    fn a_cow_backs_up_without_dropping_its_charge() {
        let mut bot = sample_bot();
        let blocks = [barricade(5.0)];
        let _ = behavior::step_tracks(
            Vec3::ZERO,
            bot.radius,
            0.0,
            bot.side,
            0.1,
            &blocks,
            &mut bot.avoidance,
        );
        assert!(
            bot.avoidance.is_some(),
            "the setup needs a route to abandon"
        );
        bot.drift.backing = behavior::UNSTICK.reverse;
        bot.drift.attempt = Vec2::new(0.0, 1.0);
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Visuals::default())
            .add_message::<JoltCue>()
            .add_systems(Update, (drive_zap_bots, charge_jolts).chain());
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.8, 4.0)));
        let cow = app
            .world_mut()
            .spawn((bot, Transform::from_xyz(0.0, 0.46, 0.0)))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.update();
        let moved = app.world().get::<Transform>(cow).unwrap().translation;
        assert!(
            moved.z < -0.05,
            "a stalled cow backs up the road, z {}",
            moved.z
        );
        let bot = app.world().get::<ZapBot>(cow).unwrap();
        assert!(bot.voiced, "the charge keeps rising through the backup");
        assert!(bot.charge > 0.0);
        assert!(
            bot.avoidance.is_none(),
            "the cow must replan after its backup"
        );
    }

    #[test]
    fn the_whine_starts_when_the_player_steps_inside_the_jolt() {
        let cow = drive_one_cow(Some(Vec3::new(0.0, 0.8, 6.0)), 200);
        assert!(cow.voiced);
        assert!(cow.charge > 0.0);
        assert_eq!(cow.bolts, 0);
    }

    #[test]
    fn a_cow_still_fires_along_its_facing_when_the_lane_is_empty() {
        let fired = discharge_once(nearly_charged(), None);
        assert!(
            fired.direction.dot(Vec3::Z) > 0.9,
            "empty lane should throw the jolt downscreen, got {}",
            fired.direction
        );
        assert_bolt_faces_its_shot(&fired);
    }

    fn nearly_charged() -> ZapBot {
        let mut bot = sample_bot();
        bot.voiced = true;
        bot.charge = jolt().charge.as_secs_f32() - 0.02;
        bot
    }

    struct FiredBolt {
        cow: Vec3,
        direction: Vec3,
        rotation: Quat,
        origin: Vec3,
        damage: u32,
        mesh_offset: Vec3,
    }

    struct DrivenCow {
        voiced: bool,
        charge: f32,
        bolts: usize,
    }

    fn drive_one_cow(player: Option<Vec3>, millis: u64) -> DrivenCow {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Visuals::default())
            .add_message::<JoltCue>()
            .add_systems(Update, (drive_zap_bots, charge_jolts).chain());
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        if let Some(player) = player {
            app.world_mut()
                .spawn((Player, Transform::from_translation(player)));
        }
        let cow = app
            .world_mut()
            .spawn((sample_bot(), Transform::from_xyz(0.0, 0.46, 0.0)))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(millis));
        app.update();
        let (voiced, charge) = {
            let bot = app.world().get::<ZapBot>(cow).unwrap();
            (bot.voiced, bot.charge)
        };
        let bolts = app
            .world_mut()
            .query::<&Projectile>()
            .iter(app.world())
            .count();
        DrivenCow {
            voiced,
            charge,
            bolts,
        }
    }

    fn discharge_once(bot: ZapBot, player: Option<Vec3>) -> FiredBolt {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Visuals::default())
            .add_message::<JoltCue>()
            .add_systems(Update, (drive_zap_bots, charge_jolts).chain());
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        if let Some(player) = player {
            app.world_mut()
                .spawn((Player, Transform::from_translation(player)));
        }
        let cow = app
            .world_mut()
            .spawn((bot, Transform::from_xyz(0.0, 0.46, 0.0)))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(50));
        app.update();
        let cow_position = app.world().get::<Transform>(cow).unwrap().translation;
        let mut bolts = app
            .world_mut()
            .query::<(Entity, &Projectile, &Transform, &Damage)>();
        let (bolt_entity, direction, origin, rotation, damage) = {
            let (entity, bolt, transform, damage) = bolts.iter(app.world()).next().expect("a jolt");
            (
                entity,
                bolt.direction,
                transform.translation,
                transform.rotation,
                damage.0,
            )
        };
        let mut meshes = app.world_mut().query::<(&Transform, &ChildOf)>();
        let mesh_offset = meshes
            .iter(app.world())
            .find(|(_, child)| child.parent() == bolt_entity)
            .expect("bolt mesh")
            .0
            .translation;
        assert!(
            app.world().get::<Visibility>(bolt_entity).is_some(),
            "the jolt root needs Visibility so the mesh can inherit it"
        );
        assert!(
            app.world()
                .get::<InheritedVisibility>(bolt_entity)
                .is_some()
        );
        FiredBolt {
            cow: cow_position,
            direction,
            rotation,
            origin,
            damage,
            mesh_offset,
        }
    }

    fn assert_bolt_faces_its_shot(fired: &FiredBolt) {
        assert_eq!(fired.damage, jolt().damage);
        let along = fired.rotation * Vec3::Z;
        assert!(
            along.dot(fired.direction) > 0.999,
            "bolt mesh +Z {along} should follow the shot {}",
            fired.direction
        );
        let from_cow = fired.origin - fired.cow;
        assert!(from_cow.length() > 0.9);
        assert!(from_cow.dot(fired.direction) > 0.9);
        // The mesh is parented in local space. +Z on that child is the parent's
        // shot direction, so a forward shift keeps the tail at the prongs.
        assert!(
            (fired.mesh_offset - Vec3::Z * (visuals::ZAP_BOLT_LENGTH * 0.5)).length() < 0.001,
            "mesh offset {}",
            fired.mesh_offset
        );
    }

    #[test]
    fn a_cow_past_the_bottom_of_the_view_is_removed() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Visuals::default())
            .add_message::<JoltCue>()
            .add_systems(Update, (drive_zap_bots, charge_jolts).chain());
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        let height = cow().height();
        let gone = app
            .world_mut()
            .spawn((
                sample_bot(),
                EnemyReward {
                    kind: EnemyKind::ZapBot,
                    hit_player: false,
                },
                Transform::from_xyz(0.0, height, 20.0),
            ))
            .id();
        let staying = app
            .world_mut()
            .spawn((
                sample_bot(),
                EnemyReward {
                    kind: EnemyKind::ZapBot,
                    hit_player: false,
                },
                Transform::from_xyz(0.0, height, 2.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<ZapBot>(gone).is_none());
        assert!(app.world().get::<ZapBot>(staying).is_some());
    }

    fn sample_bot() -> ZapBot {
        ZapBot {
            radius: cow().radius(),
            lane: 0.0,
            side: 1.0,
            avoidance: None,
            drift: Drift::default(),
            charge: 0.0,
            rest: 0.0,
            flash: 0.0,
            voiced: false,
            prongs: Handle::default(),
        }
    }
}
