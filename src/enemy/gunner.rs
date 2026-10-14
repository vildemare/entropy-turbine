//! Scout, trooper, and heavy.
//!
//! The cover gait, the retreat, and the line of sight live here. What the
//! gunner can see leaves as a [`RifleAim`]. The rifle spends the burst. A
//! [`RoundFired`] comes back when a round leaves.

use std::time::Duration;

use bevy::{prelude::*, window::PrimaryWindow};

use super::behavior::{self, Drift};
use super::rifle::{Look, Rifle, RifleAim, RoundFired};
use super::{Cover, Enemy, EnemyKind, EnemyReward, EnemyState, Tactic};
use crate::combat::{self, Faction, Health, HitCircle};
use crate::player::{MovementSpeed, Player};
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::route::{self, Boulder};
use crate::world::visuals::Visuals;

/// A gunner has fallen back off the field. Its shots belong to the projectile system.
#[derive(Message, Clone, Copy, Debug)]
pub struct ShotsRecalled {
    pub owner: Entity,
}

pub fn peel_backups(
    mut started: MessageReader<behavior::BackupStarted>,
    mut enemies: Query<&mut Enemy>,
) {
    for event in started.read().copied() {
        let Ok(mut enemy) = enemies.get_mut(event.body) else {
            continue;
        };
        enemy.evade_direction = -enemy.evade_direction;
    }
}

pub fn apply_volleys(mut fired: MessageReader<RoundFired>, mut enemies: Query<&mut Enemy>) {
    for round in fired.read().copied() {
        let Ok(mut enemy) = enemies.get_mut(round.shooter) else {
            continue;
        };
        enemy.engaged = true;
        if round.burst_spent {
            enemy.evade_direction *= -1.0;
        }
    }
}

/// Cover approach and the fall back.
/// Entry and exit are plain factors on the pace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CoverGait {
    pub entry: f32,
    pub exit: f32,
    pub retreat_speed: f32,
    pub retreat_for: Duration,
}

pub(crate) const COVER_GAIT: CoverGait = CoverGait {
    entry: 1.5,
    exit: 1.5,
    retreat_speed: 6.0,
    retreat_for: Duration::from_millis(3_500),
};

#[derive(Component)]
pub(crate) struct Retreating {
    side: f32,
    rear_z: f32,
    remaining: f32,
}

/// Where a scout, trooper, or heavy enters the field.
pub(crate) struct GunnerSpawn {
    pub group_id: u32,
    pub state: EnemyState,
    pub tactic: Tactic,
    pub cover_z: f32,
    pub slot_x: f32,
    pub flank_x: f32,
    pub hold_timer: f32,
    pub side: f32,
    pub opening: f32,
    pub position: Vec3,
}

pub(crate) fn spawn(
    commands: &mut Commands,
    visuals: &Visuals,
    kind: EnemyKind,
    spawn: GunnerSpawn,
) -> Entity {
    let definition = kind.definition();
    let actor = commands
        .spawn((
            Enemy {
                group_id: spawn.group_id,
                engaged: false,
                state: spawn.state,
                tactic: spawn.tactic,
                radius: definition.radius(),
                height: definition.height(),
                cover_z: spawn.cover_z,
                slot_x: spawn.slot_x,
                flank_x: spawn.flank_x,
                hold_timer: spawn.hold_timer,
                evade_direction: spawn.side,
                sight_blocked: false,
                drift: Drift::default(),
            },
            Rifle::from_spec(definition.rifle(), spawn.opening),
            Health(definition.health()),
            definition.runtime_abilities(),
            HitCircle {
                radius: definition.radius(),
                faction: Faction::Enemy,
                swell: true,
            },
            EnemyReward {
                kind,
                hit_player: false,
            },
            MovementSpeed(definition.speed()),
            Transform::from_translation(spawn.position),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .id();
    spawn_visual(commands, visuals, actor, kind);
    actor
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
            Option<&Rifle>,
            Option<&mut Retreating>,
        ),
        (With<Enemy>, Without<Player>),
    >,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs();
    for (mut transform, mut enemy, speed, rifle, retreating) in &mut enemies {
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
                let reloading = rifle.is_some_and(Rifle::reloading);
                let holding = rifle.is_some_and(Rifle::holding_fresh_burst);
                let sight = rifle.map(|rifle| rifle.sight).unwrap_or(f32::MAX);
                if reloading || enemy.sight_blocked {
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
                } else if distance > sight || distance > 5.5 && holding {
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
                    let max_step = COVER_GAIT.retreat_speed * dt;
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
    let step =
        behavior::reverse_step(&enemy.drift, enemy.evade_direction, max_step).unwrap_or(desired);
    enemy.drift.step = Vec2::new(step.x, step.z);
    transform.translation += step;
}

fn step_toward(position: Vec3, target: Vec3, max_step: f32) -> Vec3 {
    (target - position).clamp_length_max(max_step)
}

pub fn shoot_enemies(
    session: Res<Session>,
    players: Query<&Transform, (With<Player>, Without<Enemy>)>,
    covers: Query<(&Transform, &Cover), Without<Enemy>>,
    boulders: Query<&Transform, (With<Boulder>, Without<Enemy>)>,
    mut aims: MessageWriter<RifleAim>,
    mut enemy_queries: ParamSet<(
        Query<(Entity, &Transform, &Enemy)>,
        Query<(Entity, &mut Transform, &mut Enemy, Option<&Rifle>)>,
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
    for (entity, mut transform, mut enemy, rifle) in enemy_queries.p1().iter_mut() {
        let Some(rifle) = rifle else {
            continue;
        };
        if enemy.state == EnemyState::Retreating {
            continue;
        }
        let Some(player) = players.iter().min_by(|a, b| {
            a.translation
                .distance_squared(transform.translation)
                .total_cmp(&b.translation.distance_squared(transform.translation))
        }) else {
            if enemy.state == EnemyState::Advancing {
                aims.write(RifleAim {
                    shooter: entity,
                    acquired: false,
                    look: Look::Unseen,
                    muzzle: transform.translation,
                    direction: Vec3::ZERO,
                    distance: 0.0,
                });
            }
            continue;
        };
        let origin = Vec3::new(transform.translation.x, 0.75, transform.translation.z);
        let destination = Vec3::new(player.translation.x, 0.75, player.translation.z);
        let direction = (destination - origin).normalize_or_zero();
        let in_range = direction != Vec3::ZERO
            && origin.distance_squared(destination) <= rifle.sight * rifle.sight;
        let (look, muzzle) = if !in_range {
            (Look::Far, origin)
        } else {
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
            if blocked_by_ally || blocked_by_cover || blocked_by_boulder {
                (Look::Blocked, start)
            } else {
                (Look::Clear, start)
            }
        };
        let mut acquired = false;
        if enemy.state != EnemyState::Advancing {
            if look != Look::Clear {
                continue;
            }
            // A clear view interrupts entry, sheltering, or the route around
            // cover. The rifle fires on this same aim.
            enemy.state = EnemyState::Advancing;
            acquired = true;
            transform.scale.y = 1.0;
            transform.translation.y = enemy.height;
        }
        if look == Look::Far {
            enemy.sight_blocked = false;
        }
        if matches!(look, Look::Clear | Look::Blocked) {
            let aim = destination - origin;
            transform.rotation = Quat::from_rotation_y(aim.x.atan2(aim.z));
            enemy.sight_blocked = look == Look::Blocked;
        }
        aims.write(RifleAim {
            shooter: entity,
            acquired,
            look,
            muzzle,
            direction,
            distance: origin.distance(destination),
        });
    }
}

pub fn cleanup_behind_camera(
    mut commands: Commands,
    mut recalled: MessageWriter<ShotsRecalled>,
    cameras: Query<(&Camera, &Transform), (With<FollowCamera>, Without<Enemy>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, Option<&Retreating>)>,
    covers: Query<(Entity, &Transform), (With<Cover>, Without<Enemy>)>,
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
        .unwrap_or(camera::framed_ground_z(camera_transform) + 12.0);
    let cover_cutoff = camera::framed_ground_z(camera_transform) + 24.0;
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
                remaining: COVER_GAIT.retreat_for.as_secs_f32(),
            });
            removed.push(entity);
        }
    }
    for owner in removed {
        recalled.write(ShotsRecalled { owner });
    }
    for (entity, transform) in &covers {
        if transform.translation.z > cover_cutoff {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_visual(commands: &mut Commands, visuals: &Visuals, actor: Entity, kind: EnemyKind) {
    let coat = match kind {
        EnemyKind::Scout => &visuals.scout_material,
        EnemyKind::Trooper => &visuals.trooper_material,
        EnemyKind::Heavy => &visuals.heavy_material,
        EnemyKind::ZapBot | EnemyKind::Grenadier => return,
    };
    let size = kind.definition().visual_scale();
    let iron = &visuals.enemy_iron_material;
    let brass = &visuals.enemy_brass_material;
    let visor = &visuals.enemy_visor_material;
    let mut part = |material: &Handle<StandardMaterial>, position: Vec3, scale: Vec3| {
        commands.spawn((
            Mesh3d(visuals.enemy_part_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position * size).with_scale(scale * size),
            ChildOf(actor),
        ));
    };
    // Coat, split legs, iron boots, and a broad helmet make the role legible
    // from the overhead camera. The brass tank and forward gun suggest machinery.
    part(coat, Vec3::ZERO, Vec3::new(0.69, 0.72, 0.42));
    for side in [-1.0, 1.0] {
        part(
            coat,
            Vec3::new(side * 0.19, -0.53, 0.0),
            Vec3::new(0.24, 0.62, 0.29),
        );
        part(
            iron,
            Vec3::new(side * 0.19, -0.77, 0.12),
            Vec3::new(0.29, 0.22, 0.38),
        );
        part(
            iron,
            Vec3::new(side * 0.43, 0.16, 0.0),
            Vec3::new(0.24, 0.27, 0.43),
        );
    }
    part(coat, Vec3::new(0.0, 0.56, 0.0), Vec3::new(0.38, 0.36, 0.35));
    part(
        iron,
        Vec3::new(0.0, 0.79, -0.02),
        Vec3::new(0.55, 0.19, 0.5),
    );
    part(
        brass,
        Vec3::new(0.0, 0.76, 0.30),
        Vec3::new(0.54, 0.06, 0.16),
    );
    part(
        visor,
        Vec3::new(0.0, 0.59, 0.24),
        Vec3::new(0.28, 0.10, 0.07),
    );
    part(
        brass,
        Vec3::new(0.0, 0.06, -0.29),
        Vec3::new(0.4, 0.56, 0.24),
    );
    part(
        iron,
        Vec3::new(0.0, -0.11, 0.25),
        Vec3::new(0.55, 0.16, 0.17),
    );
    // Gun points along local +Z; the actor rotates toward its target.
    part(
        iron,
        Vec3::new(0.48, 0.04, 0.40),
        Vec3::new(0.19, 0.20, 0.86),
    );
    part(
        brass,
        Vec3::new(0.48, 0.04, 0.88),
        Vec3::new(0.23, 0.24, 0.12),
    );
    if kind == EnemyKind::Heavy {
        part(
            brass,
            Vec3::new(-0.31, 0.23, -0.29),
            Vec3::new(0.18, 0.52, 0.23),
        );
        part(
            brass,
            Vec3::new(0.31, 0.23, -0.29),
            Vec3::new(0.18, 0.52, 0.23),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::behavior::{UNSTICK, separate_enemies};
    use super::*;
    use crate::audio::SoundBank;
    use crate::combat::{Delivery, Material, Projectile, recall_shots};
    use crate::enemy::{Rifle, fire_rifles};
    use crate::world::visuals::Visuals;

    fn trooper_rifle(opening: f32) -> Rifle {
        Rifle::from_spec(crate::character::trooper().rifle(), opening)
    }

    #[test]
    fn enemies_retreat_sideways_before_leaving_and_their_shots_stop() {
        let mut app = App::new();
        app.add_message::<ShotsRecalled>()
            .add_systems(Update, (cleanup_behind_camera, recall_shots).chain());
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
            .add_message::<RifleAim>()
            .add_message::<RoundFired>()
            .add_systems(Update, (shoot_enemies, fire_rifles, apply_volleys).chain());
        let enemy = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Retreating),
                trooper_rifle(0.0),
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
                    remaining: COVER_GAIT.retreat_for.as_secs_f32(),
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
            evade_direction: 1.0,
            sight_blocked: false,
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
            .add_message::<RifleAim>()
            .add_message::<RoundFired>()
            .add_systems(Update, (shoot_enemies, fire_rifles, apply_volleys).chain());
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
                trooper_rifle(0.8),
                Transform::from_xyz(4.0, 0.68, -24.55),
            ))
            .id();
        let sheltered = app
            .world_mut()
            .spawn((
                test_enemy(EnemyState::Holding),
                trooper_rifle(0.8),
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
        assert_eq!(
            app.world().get::<Rifle>(flanker).unwrap().shots_left,
            crate::character::trooper().rifle().burst - 1
        );
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
            .add_message::<RifleAim>()
            .add_message::<RoundFired>()
            .add_systems(
                Update,
                (shoot_enemies, fire_rifles, apply_volleys, move_enemies).chain(),
            );
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
                    evade_direction: -1.0,
                    sight_blocked: false,
                    drift: Drift::default(),
                },
                trooper_rifle(0.0),
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
        assert!(app.world().get::<Rifle>(gunner).unwrap().reloading());
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
    fn a_walker_pinned_on_cover_backs_up_then_a_shot_still_stops_it() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_message::<RifleAim>()
            .add_message::<RoundFired>()
            .add_message::<behavior::BackupStarted>()
            .add_systems(
                Update,
                (
                    move_enemies,
                    separate_enemies,
                    peel_backups,
                    shoot_enemies,
                    fire_rifles,
                    apply_volleys,
                )
                    .chain(),
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
        standing.drift.backing = UNSTICK.reverse;
        standing.drift.attempt = Vec2::new(0.0, 1.0);
        let shooter = app
            .world_mut()
            .spawn((
                standing,
                trooper_rifle(0.0),
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
            UNSTICK.reverse
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
