//! Projectiles, hits, and the point orbs left by defeated enemies.

use bevy::prelude::*;

use crate::audio::{self, SoundBank};
use crate::enemy::{Cover, Enemy, EnemyReward};
use crate::player::Player;
use crate::session::{Phase, Session};
use crate::tuning::GUNSLINGER_PLAYER;
use crate::world::route::{self, Boulder};
use crate::world::visuals::Visuals;

#[derive(Component)]
pub struct Health(pub u32);

#[derive(Component)]
pub struct Damage(pub u32);

#[derive(Component)]
pub struct Lifetime(pub f32);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Faction {
    Player,
    Enemy,
}

#[derive(Component)]
pub struct Projectile {
    pub direction: Vec3,
    pub speed: f32,
    pub radius: f32,
    pub previous: Vec3,
    pub spent: bool,
    pub faction: Faction,
    pub owner: Entity,
}

#[derive(Component, Default)]
pub struct HitCooldown(pub f32);

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

#[derive(Clone, Copy)]
enum Hit {
    Cover,
    Boulder,
    Enemy(Entity),
    Player(Entity),
    Friendly,
}

pub fn resolve_hits(
    mut session: ResMut<Session>,
    sounds: Res<SoundBank>,
    visuals: Res<Visuals>,
    mut commands: Commands,
    mut bullets: Query<(&Transform, &mut Projectile, &Damage)>,
    covers: Query<(&Transform, &Cover)>,
    boulders: Query<&Transform, With<Boulder>>,
    mut enemies: Query<
        (
            Entity,
            &Transform,
            &Enemy,
            &mut Health,
            Option<&mut EnemyReward>,
        ),
        (With<Enemy>, Without<Player>),
    >,
    mut players: Query<
        (Entity, &Transform, &mut Health, &mut HitCooldown),
        (With<Player>, Without<Enemy>),
    >,
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
        let mut hit = None;
        for (cover_transform, cover) in &covers {
            if let Some(t) = segment_cover_t(start, end, cover_transform.translation, cover) {
                if t < nearest {
                    nearest = t;
                    hit = Some(Hit::Cover);
                }
            }
        }
        for boulder in &boulders {
            if let Some(t) = segment_boulder_t(start, end, boulder) {
                if t < nearest {
                    nearest = t;
                    hit = Some(Hit::Boulder);
                }
            }
        }
        for (entity, enemy_transform, enemy, health, _) in &mut enemies {
            if health.0 == 0 || (bullet.faction == Faction::Enemy && entity == bullet.owner) {
                continue;
            }
            if let Some(t) = segment_circle_t(
                start,
                end,
                enemy_transform.translation,
                enemy.radius + bullet.radius,
            ) {
                if t < nearest {
                    nearest = t;
                    hit = Some(if bullet.faction == Faction::Player {
                        Hit::Enemy(entity)
                    } else {
                        Hit::Friendly
                    });
                }
            }
        }
        if bullet.faction == Faction::Enemy {
            for (entity, player_transform, health, _) in &mut players {
                if health.0 == 0 {
                    continue;
                }
                if let Some(t) = segment_circle_t(
                    start,
                    end,
                    player_transform.translation,
                    GUNSLINGER_PLAYER.hit_radius,
                ) {
                    if t < nearest {
                        nearest = t;
                        hit = Some(Hit::Player(entity));
                    }
                }
            }
        }
        let Some(hit) = hit else { continue };
        bullet.spent = true;
        match hit {
            Hit::Enemy(entity) => {
                if let Ok((_, enemy_transform, _, mut health, reward)) = enemies.get_mut(entity) {
                    health.0 = health.0.saturating_sub(damage.0);
                    audio::play_game(&mut commands, &sounds.enemy_hit, 0.20, 1.0);
                    if health.0 == 0 {
                        audio::play_game(&mut commands, &sounds.death, 0.18, 1.3);
                        if let Some(reward) = reward {
                            let points = reward.points();
                            commands.spawn((
                                PointOrb(points),
                                Mesh3d(visuals.orb_mesh.clone()),
                                MeshMaterial3d(visuals.orb_material.clone()),
                                Transform::from_xyz(
                                    enemy_transform.translation.x,
                                    0.35,
                                    enemy_transform.translation.z,
                                ),
                            ));
                        }
                        commands.entity(entity).despawn();
                        session.kills += 1;
                    }
                }
            }
            Hit::Player(entity) => {
                if let Ok((_, _, mut health, mut cooldown)) = players.get_mut(entity) {
                    if cooldown.0 <= 0.0 {
                        health.0 = health.0.saturating_sub(damage.0);
                        if let Ok((_, _, _, _, Some(mut reward))) = enemies.get_mut(bullet.owner) {
                            reward.hit_player = true;
                        }
                        audio::play_game(&mut commands, &sounds.player_hurt, 0.35, 1.0);
                        cooldown.0 = 0.35;
                        if health.0 == 0 {
                            audio::play_game(&mut commands, &sounds.death, 0.47, 0.85);
                            session.phase = Phase::PlayerDead;
                        }
                    }
                }
            }
            Hit::Cover | Hit::Boulder => {
                audio::play_game(&mut commands, &sounds.impact, 0.13, 1.0);
            }
            Hit::Friendly => {
                audio::play_game(&mut commands, &sounds.enemy_hit, 0.09, 1.0);
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
    use crate::enemy::{AttackPhase, EnemyState, Tactic};

    #[test]
    fn small_damage_reduces_health_without_killing_player() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, resolve_hits);
        let player = app
            .world_mut()
            .spawn((
                Player,
                Health(1000),
                HitCooldown::default(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            Projectile {
                direction: Vec3::Z,
                speed: 16.0,
                radius: 0.17,
                previous: Vec3::new(0.0, 0.75, -1.0),
                spent: false,
                faction: Faction::Enemy,
                owner: Entity::PLACEHOLDER,
            },
            Damage(100),
            Transform::from_xyz(0.0, 0.75, 0.0),
        ));
        app.update();
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 900);
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
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
        }
    }

    #[test]
    fn enemy_shot_is_absorbed_by_an_ally_in_front_of_the_player() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, resolve_hits);
        let shooter = app
            .world_mut()
            .spawn((enemy(), Health(2), Transform::from_xyz(0.0, 0.5, -5.0)))
            .id();
        let ally = app
            .world_mut()
            .spawn((enemy(), Health(2), Transform::from_xyz(0.0, 0.5, -2.0)))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Player,
                Health(5),
                HitCooldown::default(),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        let shot = app
            .world_mut()
            .spawn((
                Projectile {
                    direction: Vec3::Z,
                    speed: 16.0,
                    radius: 0.17,
                    previous: Vec3::new(0.0, 0.75, -4.0),
                    spent: false,
                    faction: Faction::Enemy,
                    owner: shooter,
                },
                Damage(1),
                Transform::from_xyz(0.0, 0.75, 0.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
        assert_eq!(app.world().get::<Health>(ally).unwrap().0, 2);
        assert_eq!(app.world().get::<Health>(player).unwrap().0, 5);
    }

    #[test]
    fn barricade_stops_a_player_shot_before_it_reaches_an_enemy() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, resolve_hits);
        let enemy = app
            .world_mut()
            .spawn((enemy(), Health(2), Transform::from_xyz(0.0, 0.5, -4.0)))
            .id();
        app.world_mut().spawn((
            Cover {
                half_width: 2.7,
                half_depth: 0.5,
                height: 1.1,
            },
            Transform::from_xyz(0.0, 0.55, -2.0),
        ));
        let shot = app
            .world_mut()
            .spawn((
                Projectile {
                    direction: Vec3::NEG_Z,
                    speed: 34.0,
                    radius: 0.17,
                    previous: Vec3::new(0.0, 0.75, 0.0),
                    spent: false,
                    faction: Faction::Player,
                    owner: Entity::PLACEHOLDER,
                },
                Damage(1),
                Transform::from_xyz(0.0, 0.75, -5.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
        assert_eq!(app.world().get::<Health>(enemy).unwrap().0, 2);
    }

    #[test]
    fn side_boulder_absorbs_a_swept_shot() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, resolve_hits);
        app.world_mut().spawn((
            Boulder(0),
            Transform::from_xyz(14.7, 0.7, 0.0)
                .with_rotation(Quat::from_rotation_y(0.16))
                .with_scale(Vec3::new(1.2, 1.0, 1.0)),
        ));
        let shot = app
            .world_mut()
            .spawn((
                Projectile {
                    direction: Vec3::X,
                    speed: 34.0,
                    radius: 0.17,
                    previous: Vec3::new(12.0, 0.75, 0.0),
                    spent: false,
                    faction: Faction::Player,
                    owner: Entity::PLACEHOLDER,
                },
                Damage(1),
                Transform::from_xyz(17.0, 0.75, 0.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);
    }
}
