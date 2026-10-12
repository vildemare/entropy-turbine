//! Slow tracked machines that cross the field and throw a charged jolt.
//!
//! They are not the gunner squads and not the Entropy Turbine bosses. A cow
//! looks a short distance along its lane and slides to the clearer side of a
//! barricade instead of stopping on it. The prongs stay dark until a player
//! enters the jolt's reach, and that entry starts the whine. Local +Z is the
//! direction of travel. A finished charge fires toward the nearest player, or
//! along local +Z when the lane is empty.

use std::time::Duration;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::behavior::{self, Drift};
use super::{Cover, EnemyKind, EnemyReward};
use crate::audio::{self, SoundBank};
use crate::character::{CharacterDefinition, ZapperSpec};
use crate::combat::{Damage, Delivery, Faction, Health, HitCircle, Lifetime, Material, Projectile};
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

struct Block {
    center: Vec3,
    half_width: f32,
    half_depth: f32,
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
    let center_z = camera.translation.z - camera::CAMERA_BACK_OFFSET;
    let z = center_z - 32.0;
    let lane = director.random() * 1.6 - 0.8;
    let side = if director.random() < 0.5 { -1.0 } else { 1.0 };
    let mut x = route::clamp_actor_x(route::centerline_x(z) + lane * 8.0, z, hull.radius());
    let blocks = cover_blocks(&covers);
    for _ in 0..5 {
        if !blocked(Vec3::new(x, hull.height(), z), hull.radius(), &blocks) {
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
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<ZapBot>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    covers: Query<(&Transform, &Cover), Without<ZapBot>>,
    mut bots: Query<(Entity, &mut Transform, &mut ZapBot), Without<Player>>,
    players: Query<&Transform, (With<Player>, Without<ZapBot>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
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
    let speed = cow().speed();

    for (entity, mut transform, mut bot) in &mut bots {
        if transform.translation.z > exit_z {
            commands.entity(entity).despawn();
            continue;
        }
        let position = transform.translation;
        let lane_x = route::centerline_x(position.z + 6.0) + bot.lane * 7.0;
        let max_step = speed * dt;
        let (planned, chosen_side) =
            choose_step(position, bot.radius, lane_x, bot.side, max_step, &blocks);
        // A cow keeps driving through the backup. It does not stand to shoot.
        bot.drift.from = position;
        bot.drift.fresh = true;
        bot.drift.trying = true;
        let mut next = if let Some(back) = behavior::reverse_step(&bot.drift, bot.side, max_step) {
            bot.drift.step = Vec2::new(back.x, back.z);
            position + back
        } else {
            bot.drift.step = Vec2::new(planned.x - position.x, planned.z - position.z);
            planned
        };
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
        // The whine is the charge. It starts when a player is inside the
        // jolt's reach, and a rise already underway still finishes.
        let facing = transform.rotation * Vec3::Z;
        let in_range = player_in_bolt_range(transform.translation, &player_positions);
        let tick = step_charge(&mut bot, dt, in_range);
        if tick.start_whine {
            audio::play_game(&mut commands, &sounds.zap_charge, 0.26, 1.0);
        }
        if tick.discharge {
            let aim = aim_direction(transform.translation, facing, &player_positions);
            let origin = bolt_origin(transform.translation, aim, bot.radius);
            spawn_bolt(&mut commands, &visuals, entity, origin, aim);
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

fn choose_step(
    position: Vec3,
    radius: f32,
    lane_x: f32,
    side: f32,
    distance: f32,
    blocks: &[Block],
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
        if !blocked(next, radius, blocks) {
            return (next, chosen);
        }
    }
    (position, -side)
}

/// The nearest cover the cow would clip if it kept its current line, looked up
/// early enough that the side-step reads as a turn rather than a shove.
fn obstacle_ahead<'a>(position: Vec3, radius: f32, blocks: &'a [Block]) -> Option<&'a Block> {
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

fn clearer_side(position: Vec3, block: &Block, radius: f32, prefer: f32) -> f32 {
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

fn blocked(position: Vec3, radius: f32, blocks: &[Block]) -> bool {
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

fn cover_blocks(covers: &Query<(&Transform, &Cover), Without<ZapBot>>) -> Vec<Block> {
    covers
        .iter()
        .map(|(transform, cover)| Block {
            center: transform.translation,
            half_width: cover.half_width,
            half_depth: cover.half_depth,
        })
        .collect()
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

    use super::*;

    fn barricade(z: f32) -> Block {
        Block {
            center: Vec3::new(0.0, 0.0, z),
            half_width: 2.7,
            half_depth: 0.5,
        }
    }

    #[test]
    fn an_open_lane_carries_a_cow_downscreen() {
        let (next, side) = choose_step(Vec3::ZERO, 0.9, 0.0, 1.0, 0.4, &[]);
        assert!(next.z > 0.2);
        assert_eq!(side, 1.0);
    }

    #[test]
    fn a_barricade_ahead_pushes_the_cow_to_one_side() {
        let blocks = [barricade(1.2)];
        let start = Vec3::new(0.0, 0.0, -0.5);
        let (next, side) = choose_step(start, 0.9, 0.0, 1.0, 0.4, &blocks);
        assert!(next.x.abs() > 0.05, "x {}", next.x);
        assert!(next.z > start.z);
        assert_eq!(side.abs(), 1.0);
        assert!(!blocked(next, 0.9, &blocks));
    }

    #[test]
    fn a_cow_turns_out_several_metres_before_the_barricade() {
        let start = Vec3::new(0.0, 0.0, -6.0);
        let blocks = [barricade(0.0)];
        let (straight, _) = choose_step(start, 0.9, 0.0, 1.0, 0.5, &[]);
        let (next, side) = choose_step(start, 0.9, 0.0, 1.0, 0.5, &blocks);
        assert!(next.x.abs() > straight.x.abs() + 0.08, "x {}", next.x);
        assert!(next.z > start.z);
        assert_eq!(side.abs(), 1.0);
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
        bot.drift.backing = behavior::UNSTICK.reverse;
        bot.drift.attempt = Vec2::new(0.0, 1.0);
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(SoundBank::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Visuals::default())
            .add_systems(Update, drive_zap_bots);
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
            .add_systems(Update, drive_zap_bots);
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
            .add_systems(Update, drive_zap_bots);
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
            .add_systems(Update, drive_zap_bots);
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
            drift: Drift::default(),
            charge: 0.0,
            rest: 0.0,
            flash: 0.0,
            voiced: false,
            prongs: Handle::default(),
        }
    }
}
