//! How a living enemy moves, keeps its distance, aims, and leaves the view.

use bevy::{prelude::*, window::PrimaryWindow};

use super::{AttackPhase, Cover, Enemy, EnemyKind, EnemyReward, EnemyState, Tactic};
use crate::audio::{self, SoundBank};
use crate::combat::{self, Damage, Faction, Lifetime, Projectile};
use crate::player::{MovementSpeed, Player};
use crate::session::{Phase, Session};
use crate::tuning::ENEMY_BEHAVIOR;
use crate::world::camera::{self, FollowCamera};
use crate::world::route::{self, Boulder};
use crate::world::visuals::Visuals;

#[derive(Component)]
pub struct Retreating {
    side: f32,
    rear_z: f32,
    remaining: f32,
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
                move_toward(
                    &mut transform.translation,
                    target,
                    speed.0 * ENEMY_BEHAVIOR.cover_entry_speed_factor * dt,
                );
                if transform.translation.distance_squared(target) < 0.18 * 0.18 {
                    enemy.state = EnemyState::Holding;
                    transform.scale.y = 0.58;
                    transform.translation.y = enemy.height * 0.58;
                }
            }
            EnemyState::Holding => {
                enemy.hold_timer -= dt;
                if enemy.hold_timer <= 0.0 {
                    enemy.state = EnemyState::Flanking;
                    transform.scale.y = 1.0;
                    transform.translation.y = enemy.height;
                }
            }
            EnemyState::Flanking => {
                let target = Vec3::new(enemy.flank_x, enemy.height, enemy.cover_z - 1.55);
                move_toward(
                    &mut transform.translation,
                    target,
                    speed.0 * ENEMY_BEHAVIOR.cover_exit_speed_factor * dt,
                );
                if transform.translation.distance_squared(target) < 0.18 * 0.18 {
                    enemy.state = EnemyState::Exiting;
                }
            }
            EnemyState::Exiting => {
                transform.translation.z += speed.0 * ENEMY_BEHAVIOR.cover_exit_speed_factor * dt;
                if transform.translation.z > enemy.cover_z + 1.7 {
                    enemy.state = EnemyState::Advancing;
                }
            }
            EnemyState::Advancing => {
                let Some(player) = target_player else {
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
                    transform.translation += direction * speed.0 * dt;
                    if (transform.translation.x - route::centerline_x(transform.translation.z))
                        .abs()
                        > route::ROAD_HALF_WIDTH - enemy.radius - 0.5
                    {
                        enemy.evade_direction *= -1.0;
                    }
                } else if distance > ENEMY_BEHAVIOR.sight_range
                    || distance > 5.5
                        && enemy.fire_timer > 0.0
                        && enemy.shots_left == ENEMY_BEHAVIOR.burst_shots
                {
                    let direction = match enemy.tactic {
                        Tactic::Direct => horizontal.normalize_or_zero(),
                        Tactic::Straight => Vec3::Z,
                    };
                    transform.translation += direction * speed.0 * dt;
                }
            }
            EnemyState::Retreating => {
                if let Some(mut retreat) = retreating {
                    retreat.remaining -= dt;
                    transform.scale.y = 1.0;
                    transform.translation.y = enemy.height;
                    transform.translation.x += retreat.side * ENEMY_BEHAVIOR.retreat_speed * dt;
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

fn move_toward(position: &mut Vec3, target: Vec3, max_step: f32) {
    let delta = target - *position;
    *position += delta.clamp_length_max(max_step);
}

pub fn separate_enemies(
    mut enemies: Query<(&mut Transform, &Enemy), With<Enemy>>,
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
    for (mut transform, enemy) in &mut enemies {
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
                enemy.shots_left = ENEMY_BEHAVIOR.burst_shots;
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
                > ENEMY_BEHAVIOR.sight_range * ENEMY_BEHAVIOR.sight_range
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
            enemy.shots_left = ENEMY_BEHAVIOR.burst_shots;
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
        let weapon = reward
            .map(|reward| reward.kind)
            .unwrap_or(EnemyKind::Trooper)
            .profile();
        commands.spawn((
            Projectile {
                direction,
                speed: weapon.shot_speed,
                radius: weapon.shot_radius,
                previous: start,
                spent: false,
                faction: Faction::Enemy,
                owner: entity,
            },
            Damage(weapon.shot_damage),
            Lifetime(2.5),
            Mesh3d(visuals.bullet_mesh.clone()),
            MeshMaterial3d(visuals.enemy_bullet_material.clone()),
            Transform::from_translation(start),
        ));
        let volume = (0.26 - origin.distance(destination) * 0.004).clamp(0.08, 0.26);
        let speed = 0.94 + (ENEMY_BEHAVIOR.burst_shots - enemy.shots_left) as f32 * 0.04;
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
                remaining: ENEMY_BEHAVIOR.retreat_timeout,
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
                    remaining: ENEMY_BEHAVIOR.retreat_timeout,
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
            shots_left: ENEMY_BEHAVIOR.burst_shots,
            evade_direction: 1.0,
            sight_blocked: false,
            reload_duration: 1.5,
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
        assert_eq!(flanker_state.shots_left, ENEMY_BEHAVIOR.burst_shots - 1);
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
                    shots_left: ENEMY_BEHAVIOR.burst_shots,
                    evade_direction: -1.0,
                    sight_blocked: false,
                    reload_duration: 1.5,
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
}
