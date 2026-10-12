//! Projectiles, hits, and the point orbs left by defeated enemies.
//!
//! [`resolve_hits`] reports the nearest body a round met. [`deliver_hits`]
//! walks that body's abilities. [`present_notices`] carries out what those
//! abilities asked for.

use bevy::prelude::*;

use crate::ability::{self, Notice, Struck};
use crate::audio::{self, SoundBank};
use crate::enemy::{Cover, Drift, Enemy, EnemyReward};
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

#[derive(Component)]
pub struct PointOrb(pub u32);

pub fn collect_orbs(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut commands: Commands,
    players: Query<&Transform, With<Player>>,
    mut orbs: Query<(Entity, &mut Transform, &PointOrb), Without<Player>>,
    camera: Query<&Transform, (With<crate::world::camera::FollowCamera>, Without<PointOrb>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let rear = camera
        .iter()
        .next()
        .map(|camera| camera.translation.z + 10.0);
    for (entity, mut transform, orb) in &mut orbs {
        let mut collected = false;
        for player in &players {
            let delta = Vec2::new(
                player.translation.x - transform.translation.x,
                player.translation.z - transform.translation.z,
            );
            if delta.length() < 1.2 {
                session.points += orb.0;
                collected = true;
                break;
            }
            if delta.length() < 3.2 {
                let step = delta.normalize_or_zero() * 7.0 * time.delta_secs();
                transform.translation.x += step.x;
                transform.translation.z += step.y;
            }
        }
        if collected || rear.is_some_and(|rear| transform.translation.z > rear) {
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
    mut strikes: MessageWriter<Struck>,
    mut bullets: Query<(&Transform, &mut Projectile, &Damage)>,
    covers: Query<(Entity, &Transform, &Cover)>,
    boulders: Query<(Entity, &Transform), With<Boulder>>,
    bodies: Query<(Entity, &Transform, &HitCircle, &Health), Without<Projectile>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (transform, mut bullet, damage) in &mut bullets {
        if bullet.spent {
            continue;
        }
        let start = bullet.previous;
        let end = transform.translation;
        let mut nearest = 1.01;
        let mut target = None;
        for (entity, cover_transform, cover) in &covers {
            if let Some(t) = segment_cover_t(start, end, cover_transform.translation, cover) {
                if t < nearest {
                    nearest = t;
                    target = Some(entity);
                }
            }
        }
        for (entity, boulder) in &boulders {
            if let Some(t) = segment_boulder_t(start, end, boulder) {
                if t < nearest {
                    nearest = t;
                    target = Some(entity);
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
                }
            }
        }
        let Some(target) = target else {
            continue;
        };
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

pub fn present_notices(
    mut notices: MessageReader<Notice>,
    mut commands: Commands,
    mut session: ResMut<Session>,
    sounds: Res<SoundBank>,
    visuals: Res<Visuals>,
    mut rewards: Query<&mut EnemyReward>,
) {
    for notice in notices.read().copied() {
        match notice {
            Notice::Hurt { player: true, by } => {
                if let Ok(mut reward) = rewards.get_mut(by) {
                    reward.hit_player = true;
                }
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
                audio::play_game(&mut commands, &sounds.death, 0.18, 1.3);
                if let Ok(reward) = rewards.get(target) {
                    let points = reward.points();
                    commands.spawn((
                        PointOrb(points),
                        Mesh3d(visuals.orb_mesh.clone()),
                        MeshMaterial3d(visuals.orb_material.clone()),
                        Transform::from_xyz(at.x, 0.35, at.z),
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
            Notice::Shield => {
                audio::play_game(&mut commands, &sounds.shield_hit, 0.22, 1.0);
            }
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
    use crate::enemy::{AttackPhase, EnemyKind, EnemyReward, EnemyState, Tactic};

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
    fn nearby_orb_adds_its_value_once() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .add_systems(Update, collect_orbs);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.75, 0.0)));
        let orb = app
            .world_mut()
            .spawn((PointOrb(4), Transform::from_xyz(0.5, 0.35, 0.0)))
            .id();
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
            fire_timer: 0.0,
            attack_phase: AttackPhase::Burst,
            shots_left: 3,
            evade_direction: 1.0,
            sight_blocked: false,
            reload_duration: 1.55,
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
