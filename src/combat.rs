//! Projectiles, hits, and the point orbs left by defeated enemies.
//!
//! [`resolve_hits`] reports the nearest body a round met. [`deliver_hits`]
//! walks that body's abilities. [`present_notices`] carries out what those
//! abilities asked for, including the short burst where a machine died.

use std::time::Duration;

use bevy::prelude::*;

use crate::ability::{self, Notice, Struck};
use crate::audio::{self, SoundBank};
use crate::enemy::{Cover, Drift, Enemy, EnemyReward, ShotsRecalled};
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::world::route::{self, Boulder};
use crate::world::visuals::Visuals;

pub use crate::ability::{
    Abilities, Ability, Delivery, EnergyShield, Faction, Health, HitCooldown, Material,
};

/// The circle a projectile tests.
///
/// Machines set `swell`, so the round's own radius is added. The player's
/// circle is already the full reach.
#[derive(Component, Clone, Copy, Debug)]
pub struct HitCircle {
    pub radius: f32,
    pub faction: Faction,
    pub swell: bool,
}

/// Projectile damage on the shared combat scale where player health starts at 1000.
#[derive(Component)]
pub struct Damage(pub u32);

#[derive(Component)]
pub struct Lifetime(pub f32);

#[derive(Component)]
pub struct Projectile {
    pub direction: Vec3,
    pub speed: f32,
    pub radius: f32,
    pub previous: Vec3,
    pub spent: bool,
    pub faction: Faction,
    pub material: Material,
    pub delivery: Delivery,
    pub owner: Entity,
}

/// Remaining shallow-angle surface bounces. Ordinary and enemy rounds do not
/// carry this component.
#[derive(Component)]
pub struct Ricochet(pub u8);

const HIT_FLASH_LIFE: f32 = 0.09;
const BLAST_FLASH_LIFE: f32 = 0.24;
const DEATH_BLAST_RADIUS: f32 = 3.0;
const DEATH_BLAST_INNER_DAMAGE: u32 = 50;
const DEATH_BLAST_EDGE_DAMAGE: u32 = 20;
const BLAST_PUSH_TIME: f32 = 0.22;
const BLAST_RETURN_SPEED: f32 = 4.2;

/// A short unshadowed light attached to a hit, shield break, or destroyed
/// machine. Its intensity falls to zero before the entity is removed.
#[derive(Component)]
pub struct CombatFlash {
    remaining: f32,
    lifetime: f32,
    intensity: f32,
}

/// A robot pushed out of place by a destroyed machine. It first follows the
/// blast impulse, then walks back to the position its behavior expected before
/// any of its normal movement or attacks resume.
#[derive(Component, Clone, Copy, Debug)]
pub struct BlastRecovery {
    expected: Vec3,
    pushed_from: Vec3,
    pushed_to: Vec3,
    age: f32,
    returning: bool,
}

/// A bounty rising off the ground. `floor` is the surface it left.
/// The value is paid once into the shared bank, so every player receives it.
/// `speed` starts at 5 m/s and gains 5 m/s every 200 ms, up to 100 m/s.
#[derive(Component)]
pub struct PointOrb {
    pub value: u32,
    pub floor: f32,
    pub speed: f32,
    boost: f32,
}

const ORB_SPEED: f32 = 5.0;
const ORB_SPEED_STEP: f32 = 5.0;
const ORB_SPEED_CAP: f32 = 100.0;
const ORB_BOOST: Duration = Duration::from_millis(200);
const ORB_RISE: f32 = 75.0;

fn rising_orb(value: u32, floor: f32) -> PointOrb {
    PointOrb {
        value,
        floor,
        speed: ORB_SPEED,
        boost: ORB_BOOST.as_secs_f32(),
    }
}

pub fn collect_orbs(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut commands: Commands,
    mut orbs: Query<(Entity, &mut Transform, &mut PointOrb)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (entity, mut transform, mut orb) in &mut orbs {
        let mut dt = time.delta_secs();
        while dt > 0.0001 {
            if orb.boost <= 0.0 {
                if orb.speed >= ORB_SPEED_CAP {
                    transform.translation.y += orb.speed * dt;
                    break;
                }
                orb.speed = (orb.speed + ORB_SPEED_STEP).min(ORB_SPEED_CAP);
                orb.boost += ORB_BOOST.as_secs_f32();
            }
            let slice = dt.min(orb.boost);
            transform.translation.y += orb.speed * slice;
            orb.boost -= slice;
            dt -= slice;
            if transform.translation.y >= orb.floor + ORB_RISE {
                break;
            }
        }
        if transform.translation.y >= orb.floor + ORB_RISE {
            session.points += orb.value;
            commands.entity(entity).despawn();
        }
    }
}

pub fn advance_hit_cooldowns(time: Res<Time>, mut players: Query<&mut HitCooldown, With<Player>>) {
    for mut cooldown in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);
    }
}

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
        transform.translation += bullet.direction * bullet.speed * time.delta_secs();
    }
}

pub fn resolve_hits(
    session: Res<Session>,
    mut commands: Commands,
    mut strikes: MessageWriter<Struck>,
    mut bullets: Query<(
        &mut Transform,
        &mut Projectile,
        &Damage,
        Option<&mut Ricochet>,
    )>,
    covers: Query<(Entity, &Transform, &Cover), Without<Projectile>>,
    boulders: Query<(Entity, &Transform), (With<Boulder>, Without<Projectile>)>,
    bodies: Query<(Entity, &Transform, &HitCircle, &Health), Without<Projectile>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut transform, mut bullet, damage, ricochet) in &mut bullets {
        if bullet.spent {
            continue;
        }
        let start = bullet.previous;
        let end = transform.translation;
        let mut nearest = 1.01;
        let mut target = None;
        let mut surface_normal = None;
        for (entity, cover_transform, cover) in &covers {
            if let Some(t) = segment_cover_t(start, end, cover_transform.translation, cover) {
                if t < nearest {
                    nearest = t;
                    target = Some(entity);
                    surface_normal = Some(cover_surface_normal(
                        start.lerp(end, t),
                        cover_transform.translation,
                        cover,
                    ));
                }
            }
        }
        for (entity, boulder) in &boulders {
            if let Some(t) = segment_boulder_t(start, end, boulder) {
                if t < nearest {
                    nearest = t;
                    target = Some(entity);
                    surface_normal = Some(boulder_surface_normal(start.lerp(end, t), boulder));
                }
            }
        }
        for (entity, body_transform, circle, health) in &bodies {
            if health.0 == 0 || entity == bullet.owner {
                continue;
            }
            // A player's own rounds are not tested against the player.
            // Allied rounds still meet other machines.
            if bullet.faction != Faction::Enemy && circle.faction != Faction::Enemy {
                continue;
            }
            let reach = if circle.swell {
                circle.radius + bullet.radius
            } else {
                circle.radius
            };
            if let Some(t) = segment_circle_t(start, end, body_transform.translation, reach) {
                if t < nearest {
                    nearest = t;
                    target = Some(entity);
                    surface_normal = None;
                }
            }
        }
        let Some(target) = target else {
            continue;
        };
        let impact = start.lerp(end, nearest.clamp(0.0, 1.0));
        spawn_impact_flash(&mut commands, impact, bullet.faction, bullet.material);
        if let (Some(normal), Some(mut ricochet)) = (surface_normal, ricochet) {
            let grazing = bullet.direction.dot(normal).abs() <= 30.0_f32.to_radians().sin();
            if ricochet.0 > 0 && bullet.faction == Faction::Player && grazing {
                ricochet.0 -= 1;
                bullet.direction = bullet.direction.reflect(normal).normalize_or_zero();
                transform.rotation =
                    Quat::from_rotation_y(bullet.direction.x.atan2(bullet.direction.z));
                let restart = impact + bullet.direction * 0.025;
                bullet.previous = restart;
                transform.translation = restart;
                strikes.write(Struck {
                    target,
                    source: bullet.owner,
                    damage: damage.0,
                    material: bullet.material,
                    delivery: bullet.delivery,
                    faction: bullet.faction,
                });
                continue;
            }
        }
        bullet.spent = true;
        strikes.write(Struck {
            target,
            source: bullet.owner,
            damage: damage.0,
            material: bullet.material,
            delivery: bullet.delivery,
            faction: bullet.faction,
        });
    }
}

fn cover_surface_normal(at: Vec3, center: Vec3, cover: &Cover) -> Vec3 {
    let x = ((at.x - center.x) / cover.half_width).abs();
    let z = ((at.z - center.z) / cover.half_depth).abs();
    if x > z {
        Vec3::X * (at.x - center.x).signum()
    } else {
        Vec3::Z * (at.z - center.z).signum()
    }
}

fn boulder_surface_normal(at: Vec3, boulder: &Transform) -> Vec3 {
    let local = boulder.rotation.inverse() * (at - boulder.translation);
    let scaled = Vec3::new(
        local.x / boulder.scale.x.max(0.001),
        local.y / boulder.scale.y.max(0.001),
        local.z / boulder.scale.z.max(0.001),
    );
    let axis = if scaled.x.abs() >= scaled.z.abs() {
        Vec3::X * scaled.x.signum()
    } else {
        Vec3::Z * scaled.z.signum()
    };
    boulder.rotation * axis
}

pub fn deliver_hits(
    mut strikes: MessageReader<Struck>,
    mut notices: MessageWriter<Notice>,
    mut bodies: Query<(
        &Abilities,
        Option<&mut Health>,
        Option<&mut HitCooldown>,
        Option<&mut EnergyShield>,
        &Transform,
    )>,
) {
    for strike in strikes.read().copied() {
        let Ok((abilities, mut health, mut grace, mut shield, transform)) =
            bodies.get_mut(strike.target)
        else {
            continue;
        };
        let mut batch = Vec::new();
        ability::walk(
            strike,
            transform.translation,
            &abilities.0,
            &mut health,
            &mut grace,
            &mut shield,
            &mut batch,
        );
        for notice in batch {
            notices.write(notice);
        }
    }
}

pub fn recall_shots(
    mut recalled: MessageReader<ShotsRecalled>,
    mut commands: Commands,
    bullets: Query<(Entity, &Projectile)>,
) {
    for recall in recalled.read().copied() {
        for (entity, projectile) in &bullets {
            if projectile.faction == Faction::Enemy && projectile.owner == recall.owner {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub fn present_notices(
    mut notices: MessageReader<Notice>,
    mut strikes: MessageWriter<Struck>,
    mut commands: Commands,
    mut session: ResMut<Session>,
    sounds: Res<SoundBank>,
    visuals: Res<Visuals>,
    rewards: Query<&EnemyReward>,
    bodies: Query<(
        Entity,
        &Transform,
        &HitCircle,
        &Health,
        &EnemyReward,
        Option<&BlastRecovery>,
    )>,
    transforms: Query<&Transform>,
) {
    for notice in notices.read().copied() {
        match notice {
            Notice::Hurt { player: true, .. } => {
                audio::play_game(&mut commands, &sounds.player_hurt, 0.35, 1.0);
            }
            Notice::Hurt { player: false, .. } => {
                audio::play_game(&mut commands, &sounds.enemy_hit, 0.20, 1.0);
            }
            Notice::Died { player: true, .. } => {
                audio::play_game(&mut commands, &sounds.death, 0.47, 0.85);
                session.phase = Phase::PlayerDead;
            }
            Notice::Died {
                player: false,
                target,
                at,
            } => {
                audio::play_game(&mut commands, &sounds.enemy_blast, 0.42, 1.0);
                audio::play_game(&mut commands, &sounds.death, 0.12, 1.08);
                spawn_wreck(&mut commands, &visuals, at);
                spawn_combat_flash(
                    &mut commands,
                    at + Vec3::Y * 0.45,
                    Color::srgb(1.0, 0.29, 0.055),
                    1_800_000.0,
                    7.0,
                    BLAST_FLASH_LIFE,
                );
                scatter_death_blast(&mut commands, &mut strikes, target, at, &bodies);
                if let Ok(reward) = rewards.get(target) {
                    let points = reward.points();
                    let floor = 0.35;
                    commands.spawn((
                        rising_orb(points, floor),
                        Mesh3d(visuals.orb_mesh.clone()),
                        MeshMaterial3d(visuals.orb_material.clone()),
                        Transform::from_xyz(at.x, floor, at.z),
                    ));
                }
                session.kills += 1;
                commands.entity(target).despawn();
            }
            Notice::Impact => {
                audio::play_game(&mut commands, &sounds.impact, 0.13, 1.0);
            }
            Notice::FriendlyStop => {
                audio::play_game(&mut commands, &sounds.enemy_hit, 0.09, 1.0);
            }
            Notice::Armor => {
                audio::play_game(&mut commands, &sounds.armor_hit, 0.24, 1.0);
            }
            Notice::Shield { target, broken } => {
                if broken {
                    audio::play_game(&mut commands, &sounds.shield_break, 0.40, 1.0);
                    if let Ok(transform) = transforms.get(target) {
                        spawn_combat_flash(
                            &mut commands,
                            transform.translation + Vec3::Y * 0.35,
                            Color::srgb(0.32, 0.72, 1.0),
                            1_250_000.0,
                            5.5,
                            0.20,
                        );
                    }
                } else {
                    audio::play_game(&mut commands, &sounds.shield_hit, 0.22, 1.0);
                }
            }
        }
    }
}

fn scatter_death_blast(
    commands: &mut Commands,
    strikes: &mut MessageWriter<Struck>,
    source: Entity,
    at: Vec3,
    bodies: &Query<(
        Entity,
        &Transform,
        &HitCircle,
        &Health,
        &EnemyReward,
        Option<&BlastRecovery>,
    )>,
) {
    for (entity, transform, circle, health, reward, recovery) in bodies.iter() {
        if entity == source || health.0 == 0 || circle.faction != Faction::Enemy {
            continue;
        }
        let offset = Vec2::new(
            transform.translation.x - at.x,
            transform.translation.z - at.z,
        );
        let distance = offset.length();
        if distance > DEATH_BLAST_RADIUS {
            continue;
        }
        let along = (distance / DEATH_BLAST_RADIUS).clamp(0.0, 1.0);
        let damage = (DEATH_BLAST_INNER_DAMAGE as f32
            + (DEATH_BLAST_EDGE_DAMAGE as f32 - DEATH_BLAST_INNER_DAMAGE as f32) * along)
            .round() as u32;
        strikes.write(Struck {
            target: entity,
            source,
            damage,
            material: Material::Explosive,
            delivery: Delivery::Kinetic,
            faction: Faction::Enemy,
        });

        let direction = if offset.length_squared() > 1.0e-6 {
            offset.normalize()
        } else {
            Vec2::X
        };
        let weight = match reward.kind {
            crate::enemy::EnemyKind::Scout => 1.0,
            crate::enemy::EnemyKind::Trooper => 0.82,
            crate::enemy::EnemyKind::Heavy => 0.48,
            crate::enemy::EnemyKind::ZapBot => 0.68,
            crate::enemy::EnemyKind::Grenadier => 0.44,
        };
        let push = (0.65 + (1.0 - along) * 1.55) * weight;
        let expected = recovery
            .map(|recovery| recovery.expected)
            .unwrap_or(transform.translation);
        let pushed_from = transform.translation;
        commands.entity(entity).insert(BlastRecovery {
            expected,
            pushed_from,
            pushed_to: pushed_from + Vec3::new(direction.x, 0.0, direction.y) * push,
            age: 0.0,
            returning: false,
        });
    }
}

fn spawn_impact_flash(commands: &mut Commands, at: Vec3, faction: Faction, material: Material) {
    let (color, intensity, range) = match (faction, material) {
        (Faction::Player, Material::Plasma) => (Color::srgb(0.56, 0.82, 1.0), 520_000.0, 3.3),
        (_, Material::Electricity) => (Color::srgb(1.0, 0.78, 0.24), 420_000.0, 3.0),
        (_, Material::Explosive) => (Color::srgb(1.0, 0.30, 0.06), 700_000.0, 4.0),
        _ => (Color::srgb(1.0, 0.22, 0.07), 180_000.0, 2.2),
    };
    spawn_combat_flash(
        commands,
        at + Vec3::Y * 0.15,
        color,
        intensity,
        range,
        HIT_FLASH_LIFE,
    );
}

fn spawn_combat_flash(
    commands: &mut Commands,
    at: Vec3,
    color: Color,
    intensity: f32,
    range: f32,
    lifetime: f32,
) {
    commands.spawn((
        CombatFlash {
            remaining: lifetime,
            lifetime,
            intensity,
        },
        PointLight {
            color,
            intensity,
            range,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(at),
    ));
}

pub fn fade_combat_flashes(
    time: Res<Time>,
    mut commands: Commands,
    mut flashes: Query<(Entity, &mut PointLight, &mut CombatFlash)>,
) {
    for (entity, mut light, mut flash) in &mut flashes {
        flash.remaining -= time.delta_secs();
        if flash.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let fade = (flash.remaining / flash.lifetime).clamp(0.0, 1.0);
        light.intensity = flash.intensity * fade * fade;
    }
}

pub fn recover_blast_displacements(
    time: Res<Time>,
    mut commands: Commands,
    mut robots: Query<(Entity, &mut Transform, &HitCircle, &mut BlastRecovery), Without<Player>>,
    covers: Query<(&Transform, &Cover), Without<BlastRecovery>>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, circle, mut recovery) in &mut robots {
        if !recovery.returning {
            recovery.age += dt;
            let t = (recovery.age / BLAST_PUSH_TIME).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            transform.translation = recovery.pushed_from.lerp(recovery.pushed_to, eased);
            if t >= 1.0 {
                recovery.returning = true;
            }
        } else {
            let delta = recovery.expected - transform.translation;
            let flat = Vec3::new(delta.x, 0.0, delta.z);
            let step = flat.clamp_length_max(BLAST_RETURN_SPEED * dt);
            transform.translation += step;
            if step.length_squared() > 1.0e-6 {
                transform.rotation = Quat::from_rotation_y(step.x.atan2(step.z));
            }
            if flat.length_squared() <= 0.08 * 0.08 {
                transform.translation = recovery.expected;
                commands.entity(entity).remove::<BlastRecovery>();
                continue;
            }
        }
        transform.translation.x = route::clamp_actor_x(
            transform.translation.x,
            transform.translation.z,
            circle.radius,
        );
        for (cover_transform, cover) in &covers {
            crate::enemy::separate_from_cover(
                &mut transform.translation,
                circle.radius,
                cover_transform.translation,
                cover,
            );
        }
    }
}

/// Earliest intersection of a horizontal projectile segment and a round actor.
pub fn segment_circle_t(start: Vec3, end: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let fx = start.x - center.x;
    let fz = start.z - center.z;
    let a = dx * dx + dz * dz;
    let c = fx * fx + fz * fz - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    if a <= f32::EPSILON {
        return None;
    }
    let b = 2.0 * (fx * dx + fz * dz);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Earliest intersection with the solid part of a barricade.
pub fn segment_cover_t(start: Vec3, end: Vec3, center: Vec3, cover: &Cover) -> Option<f32> {
    if start.y > cover.height || end.y > cover.height {
        return None;
    }
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (origin, delta, min, max) in [
        (
            start.x,
            end.x - start.x,
            center.x - cover.half_width,
            center.x + cover.half_width,
        ),
        (
            start.z,
            end.z - start.z,
            center.z - cover.half_depth,
            center.z + cover.half_depth,
        ),
    ] {
        if delta.abs() < 0.0001 {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let a = (min - origin) / delta;
            let b = (max - origin) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

/// Swept hit against a boulder's rotated and scaled visible cuboid.
pub fn segment_boulder_t(start: Vec3, end: Vec3, transform: &Transform) -> Option<f32> {
    let inverse = transform.rotation.inverse();
    let local_start = inverse * (start - transform.translation);
    let local_end = inverse * (end - transform.translation);
    let half = route::BOULDER_SIZE * transform.scale.abs() * 0.5;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (origin, delta, extent) in [
        (local_start.x, local_end.x - local_start.x, half.x),
        (local_start.y, local_end.y - local_start.y, half.y),
        (local_start.z, local_end.z - local_start.z, half.z),
    ] {
        if delta.abs() < 0.0001 {
            if origin.abs() > extent {
                return None;
            }
        } else {
            let a = (-extent - origin) / delta;
            let b = (extent - origin) / delta;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

const BURST_LIFE: Duration = Duration::from_millis(450);

/// A piece of the flash left where a machine died.
#[derive(Component)]
pub struct WreckBurst {
    age: f32,
    origin: Vec3,
    piece: WreckPiece,
}

#[derive(Clone, Copy)]
enum WreckPiece {
    Core,
    Ring,
    Shard(Vec3),
}

fn spawn_wreck(commands: &mut Commands, visuals: &Visuals, at: Vec3) {
    let origin = Vec3::new(at.x, 0.45, at.z);
    commands.spawn((
        WreckBurst {
            age: 0.0,
            origin,
            piece: WreckPiece::Core,
        },
        Mesh3d(visuals.bullet_mesh.clone()),
        MeshMaterial3d(visuals.blast_mark_material.clone()),
        Transform::from_translation(origin),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    commands.spawn((
        WreckBurst {
            age: 0.0,
            origin: Vec3::new(at.x, 0.12, at.z),
            piece: WreckPiece::Ring,
        },
        Mesh3d(visuals.blast_ring_mesh.clone()),
        MeshMaterial3d(visuals.blast_mark_material.clone()),
        Transform::from_xyz(at.x, 0.12, at.z)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        GlobalTransform::default(),
        Visibility::default(),
    ));
    for index in 0..6 {
        let angle = index as f32 * std::f32::consts::TAU / 6.0;
        let dir = Vec3::new(angle.cos(), 0.45, angle.sin());
        commands.spawn((
            WreckBurst {
                age: 0.0,
                origin,
                piece: WreckPiece::Shard(dir),
            },
            Mesh3d(visuals.enemy_part_mesh.clone()),
            MeshMaterial3d(visuals.enemy_brass_material.clone()),
            Transform::from_translation(origin).with_scale(Vec3::splat(0.16)),
            GlobalTransform::default(),
            Visibility::default(),
        ));
    }
}

pub fn bloom_bursts(
    time: Res<Time>,
    mut commands: Commands,
    mut bursts: Query<(Entity, &mut Transform, &mut WreckBurst)>,
) {
    let life = BURST_LIFE.as_secs_f32();
    let dt = time.delta_secs();
    for (entity, mut transform, mut burst) in &mut bursts {
        burst.age += dt;
        if burst.age >= life {
            commands.entity(entity).despawn();
            continue;
        }
        let t = (burst.age / life).clamp(0.0, 1.0);
        match burst.piece {
            WreckPiece::Core => {
                let scale = 1.2 + (1.0 - (1.0 - t).powi(2)) * 6.5;
                transform.scale = Vec3::splat(scale);
                transform.translation.y = burst.origin.y + t * 0.35;
            }
            WreckPiece::Ring => {
                transform.scale = Vec3::splat(0.35 + t * 2.1);
            }
            WreckPiece::Shard(dir) => {
                transform.translation = burst.origin + dir * (t * 1.7);
                transform.scale = Vec3::splat((0.22 * (1.0 - t)).max(0.04));
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
            || (transform.translation.x - route::centerline_x(transform.translation.z)).abs()
                >= route::ROAD_HALF_WIDTH + 5.0
        {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::HIT_GRACE;
    use crate::enemy::{EnemyKind, EnemyReward, EnemyState, Tactic};

    fn prepare(app: &mut App) {
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_message::<Struck>()
            .add_message::<Notice>()
            .add_systems(
                Update,
                (resolve_hits, deliver_hits, present_notices).chain(),
            );
    }

    fn round(faction: Faction, material: Material, delivery: Delivery) -> Projectile {
        Projectile {
            direction: Vec3::Z,
            speed: 16.0,
            radius: 0.17,
            previous: Vec3::ZERO,
            spent: false,
            faction,
            material,
            delivery,
            owner: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn small_damage_reduces_health_without_killing_player() {
        let mut app = App::new();
        prepare(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Player,
                Abilities::player(),
                Health(1000),
                HitCooldown::default(),
                player_circle(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        let mut shot = round(Faction::Enemy, Material::Metal, Delivery::Kinetic);
        shot.previous = Vec3::new(0.0, 0.75, -1.0);
        app.world_mut()
            .spawn((shot, Damage(100), Transform::from_xyz(0.0, 0.75, 0.0)));
        app.update();
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 900);
        assert_eq!(app.world().get::<HitCooldown>(player).unwrap().0, HIT_GRACE);
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
    }

    #[test]
    fn a_second_round_in_the_same_moment_meets_the_players_grace() {
        let mut app = App::new();
        prepare(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Player,
                Abilities::player(),
                Health(1000),
                HitCooldown::default(),
                player_circle(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        for _ in 0..2 {
            let mut shot = round(Faction::Enemy, Material::Metal, Delivery::Kinetic);
            shot.previous = Vec3::new(0.0, 0.75, -1.0);
            app.world_mut()
                .spawn((shot, Damage(100), Transform::from_xyz(0.0, 0.75, 0.0)));
        }
        app.update();
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 900);
    }

    #[test]
    fn a_defeated_enemy_bursts_and_the_flash_fades() {
        let mut app = App::new();
        prepare(&mut app);
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, bloom_bursts.after(present_notices));
        let enemy = app
            .world_mut()
            .spawn((
                EnemyReward {
                    kind: EnemyKind::Scout,
                    hit_player: false,
                },
                Transform::from_xyz(2.0, 0.5, 3.0),
            ))
            .id();
        app.world_mut().write_message(Notice::Died {
            player: false,
            target: enemy,
            at: Vec3::new(2.0, 0.5, 3.0),
        });
        app.update();

        assert!(app.world().get_entity(enemy).is_err());
        assert_eq!(app.world().resource::<Session>().kills, 1);
        let mut bursts = app.world_mut().query::<(Entity, &WreckBurst)>();
        let found: Vec<_> = bursts.iter(app.world()).map(|(entity, _)| entity).collect();
        assert_eq!(found.len(), 8);
        assert_eq!(
            app.world_mut()
                .query::<&CombatFlash>()
                .iter(app.world())
                .count(),
            1
        );
        let mut orbs = app.world_mut().query::<&PointOrb>();
        assert_eq!(orbs.iter(app.world()).count(), 1);
        let core = bursts
            .iter(app.world())
            .find_map(|(entity, burst)| matches!(burst.piece, WreckPiece::Core).then_some(entity))
            .unwrap();
        let opened = app.world().get::<Transform>(core).unwrap().scale.x;

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(200));
        app.update();
        let grown = app.world().get::<Transform>(core).unwrap().scale.x;
        assert!(grown > opened);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(400));
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&WreckBurst>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(orbs.iter(app.world()).count(), 1);
    }

    #[test]
    fn a_destroyed_machine_damages_and_displaces_nearby_robots_only() {
        let mut app = App::new();
        prepare(&mut app);
        let source = app
            .world_mut()
            .spawn((
                EnemyReward {
                    kind: EnemyKind::Scout,
                    hit_player: false,
                },
                Transform::from_xyz(0.0, 0.5, 0.0),
            ))
            .id();
        let spawn_target = |world: &mut World, x: f32| {
            world
                .spawn((
                    Abilities::enemy(),
                    Health(300),
                    body_circle(),
                    EnemyReward {
                        kind: EnemyKind::Scout,
                        hit_player: false,
                    },
                    Transform::from_xyz(x, 0.5, 0.0),
                ))
                .id()
        };
        let near = spawn_target(app.world_mut(), 1.0);
        let far = spawn_target(app.world_mut(), 4.0);
        app.world_mut().write_message(Notice::Died {
            player: false,
            target: source,
            at: Vec3::new(0.0, 0.5, 0.0),
        });

        app.update();
        assert!(app.world().get::<BlastRecovery>(near).is_some());
        assert!(app.world().get::<BlastRecovery>(far).is_none());
        app.update();
        assert_eq!(app.world().get::<Health>(near).unwrap().0, 260);
        assert_eq!(app.world().get::<Health>(far).unwrap().0, 300);
    }

    #[test]
    fn a_blast_displaced_robot_returns_to_its_expected_position() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, recover_blast_displacements);
        let robot = app
            .world_mut()
            .spawn((
                body_circle(),
                Transform::from_xyz(0.0, 0.5, 0.0),
                BlastRecovery {
                    expected: Vec3::new(0.0, 0.5, 0.0),
                    pushed_from: Vec3::new(0.0, 0.5, 0.0),
                    pushed_to: Vec3::new(1.0, 0.5, 0.0),
                    age: 0.0,
                    returning: false,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(220));
        app.update();
        assert!(app.world().get::<Transform>(robot).unwrap().translation.x > 0.9);

        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(100));
            app.update();
        }
        assert!(app.world().get::<BlastRecovery>(robot).is_none());
        assert_eq!(
            app.world().get::<Transform>(robot).unwrap().translation,
            Vec3::new(0.0, 0.5, 0.0)
        );
    }

    #[test]
    fn a_risen_orb_pays_the_shared_bank_once() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .add_systems(Update, collect_orbs);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.75, 0.0)));
        app.world_mut()
            .spawn((Player, Transform::from_xyz(8.0, 0.75, 6.0)));
        let orb = app
            .world_mut()
            .spawn((rising_orb(4, 0.35), Transform::from_xyz(3.0, 0.35, -4.0)))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.update();
        assert_eq!(app.world().resource::<Session>().points, 0);
        let early = app.world().get::<Transform>(orb).unwrap().translation.y;
        assert!((early - 0.85).abs() < 0.001);
        assert!((app.world().get::<PointOrb>(orb).unwrap().speed - 5.0).abs() < 0.001);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.update();
        let later = app.world().get::<Transform>(orb).unwrap().translation.y;
        assert!((later - 3.35).abs() < 0.001);
        assert!((app.world().get::<PointOrb>(orb).unwrap().speed - 10.0).abs() < 0.001);
        assert_eq!(app.world().resource::<Session>().points, 0);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(3_000));
        app.update();
        assert_eq!(app.world().resource::<Session>().points, 4);
        assert!(app.world().get_entity(orb).is_err());
        app.update();
        assert_eq!(app.world().resource::<Session>().points, 4);
    }

    fn player_circle() -> HitCircle {
        HitCircle {
            radius: crate::character::gunslinger().hit_radius(),
            faction: Faction::Player,
            swell: false,
        }
    }

    fn body_circle() -> HitCircle {
        HitCircle {
            radius: 0.56,
            faction: Faction::Enemy,
            swell: true,
        }
    }

    fn enemy() -> Enemy {
        Enemy {
            group_id: 0,
            engaged: false,
            state: EnemyState::Advancing,
            tactic: Tactic::Direct,
            radius: 0.56,
            height: 0.68,
            cover_z: 0.0,
            slot_x: 0.0,
            flank_x: 0.0,
            hold_timer: 0.0,
            evade_direction: 1.0,
            sight_blocked: false,
            drift: Drift::default(),
        }
    }

    #[test]
    fn enemy_shot_is_absorbed_by_an_ally_in_front_of_the_player() {
        let mut app = App::new();
        prepare(&mut app);
        let shooter = app
            .world_mut()
            .spawn((
                enemy(),
                Abilities::enemy(),
                Health(2),
                body_circle(),
                Transform::from_xyz(0.0, 0.5, -5.0),
            ))
            .id();
        let ally = app
            .world_mut()
            .spawn((
                enemy(),
                Abilities::enemy(),
                Health(2),
                body_circle(),
                Transform::from_xyz(0.0, 0.5, -2.0),
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Player,
                Abilities::player(),
                Health(5),
                HitCooldown::default(),
                player_circle(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        let mut shot = round(Faction::Enemy, Material::Metal, Delivery::Kinetic);
        shot.previous = Vec3::new(0.0, 0.75, -4.0);
        shot.owner = shooter;
        let shot = app
            .world_mut()
            .spawn((shot, Damage(1), Transform::from_xyz(0.0, 0.75, 0.0)))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
        assert_eq!(app.world().get::<Health>(ally).unwrap().0, 2);
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 5);
    }

    #[test]
    fn barricade_stops_a_player_shot_before_it_reaches_an_enemy() {
        let mut app = App::new();
        prepare(&mut app);
        let enemy = app
            .world_mut()
            .spawn((
                enemy(),
                Abilities::enemy(),
                Health(2),
                body_circle(),
                Transform::from_xyz(0.0, 0.5, -4.0),
            ))
            .id();
        app.world_mut().spawn((
            Abilities::solid(),
            Cover {
                half_width: 2.7,
                half_depth: 0.5,
                height: 1.1,
            },
            Transform::from_xyz(0.0, 0.55, -2.0),
        ));
        let mut shot = round(Faction::Player, Material::Plasma, Delivery::Energy);
        shot.direction = Vec3::NEG_Z;
        shot.speed = 34.0;
        shot.previous = Vec3::new(0.0, 0.75, 0.0);
        let shot = app
            .world_mut()
            .spawn((shot, Damage(1), Transform::from_xyz(0.0, 0.75, -5.0)))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
        assert_eq!(app.world().get::<Health>(enemy).unwrap().0, 2);
    }

    #[test]
    fn a_grazing_gunslinger_round_ricochets_but_a_steep_one_shatters() {
        let mut app = App::new();
        prepare(&mut app);
        app.world_mut().spawn((
            Abilities::solid(),
            Cover {
                half_width: 2.7,
                half_depth: 0.5,
                height: 1.1,
            },
            Transform::from_xyz(0.0, 0.55, 0.0),
        ));
        let mut grazing = round(Faction::Player, Material::Plasma, Delivery::Energy);
        grazing.previous = Vec3::new(-2.0, 0.75, 0.6);
        grazing.direction = Vec3::new(4.0, 0.0, -1.2).normalize();
        let grazing = app
            .world_mut()
            .spawn((
                grazing,
                Ricochet(2),
                Damage(200),
                Transform::from_xyz(2.0, 0.75, -0.6),
            ))
            .id();
        let mut steep = round(Faction::Player, Material::Plasma, Delivery::Energy);
        steep.previous = Vec3::new(0.0, 0.75, 2.0);
        steep.direction = Vec3::NEG_Z;
        let steep = app
            .world_mut()
            .spawn((
                steep,
                Ricochet(2),
                Damage(200),
                Transform::from_xyz(0.0, 0.75, -2.0),
            ))
            .id();

        app.update();
        let grazing_shot = app.world().get::<Projectile>(grazing).unwrap();
        assert!(!grazing_shot.spent);
        assert!(grazing_shot.direction.z > 0.0);
        assert_eq!(app.world().get::<Ricochet>(grazing).unwrap().0, 1);
        assert!(app.world().get::<Projectile>(steep).unwrap().spent);
    }

    #[test]
    fn side_boulder_absorbs_a_swept_shot() {
        let mut app = App::new();
        prepare(&mut app);
        app.world_mut().spawn((
            Abilities::solid(),
            Boulder(0),
            Transform::from_xyz(14.7, 0.7, 0.0)
                .with_rotation(Quat::from_rotation_y(0.16))
                .with_scale(Vec3::new(1.2, 1.0, 1.0)),
        ));
        let mut shot = round(Faction::Player, Material::Plasma, Delivery::Energy);
        shot.direction = Vec3::X;
        shot.speed = 34.0;
        shot.previous = Vec3::new(12.0, 0.75, 0.0);
        let shot = app
            .world_mut()
            .spawn((shot, Damage(1), Transform::from_xyz(17.0, 0.75, 0.0)))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
    }

    #[test]
    fn a_guard_in_front_of_hit_points_keeps_part_of_the_round() {
        let mut app = App::new();
        prepare(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Player,
                Abilities(vec![Ability::ReduceDamage(40), Ability::PlayerLife]),
                Health(1000),
                player_circle(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        let mut shot = round(Faction::Enemy, Material::Metal, Delivery::Kinetic);
        shot.previous = Vec3::new(0.0, 0.75, -1.0);
        app.world_mut()
            .spawn((shot, Damage(100), Transform::from_xyz(0.0, 0.75, 0.0)));
        app.update();
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 940);
    }

    #[test]
    fn one_killing_blow_counts_once_when_two_rounds_meet_the_body() {
        let mut app = App::new();
        prepare(&mut app);
        let enemy = app
            .world_mut()
            .spawn((
                enemy(),
                Abilities::enemy(),
                Health(2),
                body_circle(),
                EnemyReward {
                    kind: EnemyKind::Scout,
                    hit_player: false,
                },
                Transform::from_xyz(0.0, 0.5, -4.0),
            ))
            .id();
        for _ in 0..2 {
            let mut shot = round(Faction::Player, Material::Plasma, Delivery::Energy);
            shot.direction = Vec3::NEG_Z;
            shot.previous = Vec3::new(0.0, 0.5, 0.0);
            app.world_mut()
                .spawn((shot, Damage(2), Transform::from_xyz(0.0, 0.5, -6.0)));
        }
        app.update();
        assert!(app.world().get_entity(enemy).is_err());
        assert_eq!(app.world().resource::<Session>().kills, 1);
        assert_eq!(
            app.world_mut()
                .query::<&PointOrb>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
