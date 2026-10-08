//! The Gunslinger: intent, movement, weapon, and spawning.

use std::time::Duration;

use bevy::{prelude::*, window::PrimaryWindow};

pub mod ammo_hud;
pub mod animation;
pub mod input;

use animation::{CharacterAsset, CharacterModel};

use crate::audio::{self, SoundBank};
use crate::combat::{Damage, Faction, Health, HitCooldown, Lifetime, Projectile};
use crate::enemy::{self, Cover};
use crate::session::{Phase, Session};
use crate::tuning::{GUNSLINGER_PLAYER, GUNSLINGER_WEAPON};
use crate::world::camera;
use crate::world::route;
use crate::world::visuals::Visuals;

pub const PLAYER_Y: f32 = 0.75;

pub fn reload_wait_ms_for(purchases: u32) -> u32 {
    GUNSLINGER_WEAPON
        .starting_reload_wait_ms
        .saturating_sub(purchases.saturating_mul(GUNSLINGER_WEAPON.reload_upgrade_ms))
        .max(GUNSLINGER_WEAPON.min_reload_wait_ms)
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct KeyboardMouseControlled;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerSlot(pub u8);

#[derive(Component)]
pub struct MovementSpeed(pub f32);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub aim: Vec3,
    pub firing: bool,
}

pub fn is_shooting(intent: &PlayerIntent, weapon: &GunslingerWeapon) -> bool {
    intent.firing && weapon.can_fire()
}

#[derive(Component)]
pub struct FireCooldown(pub Duration);

#[derive(Component, Default, Clone, Copy)]
pub struct PlayerProgression {
    pub health_level: u32,
    pub capacity_level: u32,
    pub reload_level: u32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStats {
    pub max_health: u32,
    pub capacity: u8,
    pub reload_wait_ms: u32,
}

impl PlayerProgression {
    pub fn stats(self) -> PlayerStats {
        PlayerStats {
            max_health: GUNSLINGER_PLAYER.starting_health
                + self.health_level.min(
                    (GUNSLINGER_PLAYER.max_health - GUNSLINGER_PLAYER.starting_health)
                        / GUNSLINGER_PLAYER.health_per_upgrade,
                ) * GUNSLINGER_PLAYER.health_per_upgrade,
            capacity: GUNSLINGER_WEAPON.starting_capacity
                + self.capacity_level.min(u32::from(
                    GUNSLINGER_WEAPON.max_capacity - GUNSLINGER_WEAPON.starting_capacity,
                )) as u8,
            reload_wait_ms: reload_wait_ms_for(self.reload_level),
        }
    }
}

pub fn apply_progression(
    progression: PlayerProgression,
    stats: &mut PlayerStats,
    health: &mut Health,
    weapon: &mut GunslingerWeapon,
    restore: bool,
) {
    let next = progression.stats();
    health.0 = if restore {
        next.max_health
    } else {
        health
            .0
            .saturating_add(next.max_health.saturating_sub(stats.max_health))
            .min(next.max_health)
    };
    weapon.apply_stats(next);
    if restore {
        weapon.refill();
    }
    *stats = next;
}

#[derive(Clone)]
pub struct RevolverDrum {
    pub chambers: Vec<bool>,
    pub next: usize,
    pub spin_steps: u32,
}

impl RevolverDrum {
    fn new(capacity: usize) -> Self {
        Self {
            chambers: vec![true; capacity],
            next: 0,
            spin_steps: 0,
        }
    }

    fn fire(&mut self) -> bool {
        let count = self.chambers.len();
        let Some(offset) = (0..count).find(|offset| self.chambers[(self.next + offset) % count])
        else {
            return false;
        };
        let index = (self.next + offset) % count;
        self.chambers[index] = false;
        self.next = (index + 1) % count;
        let mut steps = offset + 1;
        if let Some(skip) = (0..count).find(|skip| self.chambers[(self.next + *skip) % count]) {
            self.next = (self.next + skip) % count;
            steps += skip;
        }
        self.spin_steps += steps as u32;
        true
    }

    fn load_one(&mut self) -> bool {
        let count = self.chambers.len();
        let Some(index) = (0..count)
            .map(|offset| (self.next + offset) % count)
            .find(|&index| !self.chambers[index])
        else {
            return false;
        };
        self.chambers[index] = true;
        true
    }
}

#[derive(Component)]
pub struct GunslingerWeapon {
    pub rounds: u8,
    pub capacity: u8,
    pub reload_wait_ms: u32,
    pub reload_remaining: Duration,
    pub recharging: bool,
    pub waiting_for_loader: bool,
    pub drums: [RevolverDrum; 2],
    next_gun: usize,
    next_reload_gun: usize,
}

impl Default for GunslingerWeapon {
    fn default() -> Self {
        let stats = PlayerProgression::default().stats();
        Self {
            rounds: stats.capacity,
            capacity: stats.capacity,
            reload_wait_ms: stats.reload_wait_ms,
            reload_remaining: Duration::from_millis(stats.reload_wait_ms as u64),
            recharging: false,
            waiting_for_loader: false,
            drums: [
                RevolverDrum::new(usize::from(stats.capacity.div_ceil(2))),
                RevolverDrum::new(usize::from(stats.capacity / 2)),
            ],
            next_gun: 0,
            next_reload_gun: 0,
        }
    }
}

impl GunslingerWeapon {
    pub fn can_fire(&self) -> bool {
        self.rounds > 0 && !self.recharging
    }

    pub fn refill(&mut self) {
        for drum in &mut self.drums {
            drum.chambers.fill(true);
        }
        self.rounds = self.capacity;
        self.reload_remaining = Duration::from_millis(self.reload_wait_ms as u64);
        self.recharging = false;
        self.waiting_for_loader = false;
    }

    pub fn apply_stats(&mut self, stats: PlayerStats) {
        let sizes = [
            usize::from(stats.capacity.div_ceil(2)),
            usize::from(stats.capacity / 2),
        ];
        for (drum, size) in self.drums.iter_mut().zip(sizes) {
            drum.chambers.resize(size, true);
            drum.next %= size;
            if let Some(offset) =
                (0..size).find(|offset| drum.chambers[(drum.next + *offset) % size])
            {
                drum.next = (drum.next + offset) % size;
            }
            drum.spin_steps = drum.next as u32;
        }
        self.capacity = stats.capacity;
        self.rounds = self
            .drums
            .iter()
            .flat_map(|drum| &drum.chambers)
            .filter(|&&loaded| loaded)
            .count() as u8;
        self.reload_wait_ms = stats.reload_wait_ms;
        if self.waiting_for_loader {
            self.reload_remaining = self
                .reload_remaining
                .min(Duration::from_millis(self.reload_wait_ms as u64));
        }
    }

    fn advance_reload(&mut self, delta: Duration) {
        if !self.waiting_for_loader && !self.recharging {
            return;
        }
        let mut delta = delta;
        while (self.waiting_for_loader || self.recharging) && delta >= self.reload_remaining {
            delta -= self.reload_remaining;
            if self.waiting_for_loader {
                self.waiting_for_loader = false;
                self.recharging = true;
                self.reload_remaining =
                    Duration::from_millis(GUNSLINGER_WEAPON.quickload_chamber_ms);
            } else {
                self.load_round();
                if self.rounds == self.capacity {
                    self.recharging = false;
                    self.reload_remaining = Duration::from_millis(self.reload_wait_ms as u64);
                    break;
                }
                self.reload_remaining =
                    Duration::from_millis(GUNSLINGER_WEAPON.quickload_chamber_ms);
            }
        }
        if self.waiting_for_loader || self.recharging {
            self.reload_remaining -= delta;
        }
    }

    fn fire_round(&mut self) -> Option<usize> {
        let gun = if self.drums[self.next_gun].fire() {
            self.next_gun
        } else {
            let other = 1 - self.next_gun;
            if !self.drums[other].fire() {
                return None;
            }
            other
        };
        self.rounds -= 1;
        self.next_gun = 1 - gun;
        self.recharging = self.rounds == 0;
        self.waiting_for_loader = true;
        self.reload_remaining = Duration::from_millis(self.reload_wait_ms as u64);
        Some(gun)
    }

    fn load_round(&mut self) {
        let gun = if self.drums[self.next_reload_gun].load_one() {
            self.next_reload_gun
        } else {
            let other = 1 - self.next_reload_gun;
            if !self.drums[other].load_one() {
                return;
            }
            other
        };
        self.rounds += 1;
        self.next_reload_gun = 1 - gun;
    }
}

pub fn spawn_player(commands: &mut Commands, character: &CharacterAsset, position: Vec3) {
    let build = PlayerProgression::default();
    let stats = build.stats();
    let actor = commands
        .spawn((
            Player,
            LocalPlayer,
            KeyboardMouseControlled,
            PlayerSlot(0),
            PlayerIntent::default(),
            MovementSpeed(GUNSLINGER_PLAYER.move_speed),
            FireCooldown(Duration::ZERO),
            build,
            stats,
            GunslingerWeapon::default(),
            HitCooldown::default(),
            Health(stats.max_health),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id();
    commands.spawn((
        CharacterModel,
        WorldAssetRoot(character.scene.clone()),
        Transform::from_xyz(0.0, -PLAYER_Y, 0.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            .with_scale(Vec3::splat(0.8)),
        ChildOf(actor),
    ));
}

pub fn clear_inactive_intents(
    session: Res<Session>,
    mut players: Query<&mut PlayerIntent, With<Player>>,
) {
    if session.phase != Phase::Playing {
        for mut intent in &mut players {
            intent.movement = Vec2::ZERO;
            intent.firing = false;
        }
    }
}

pub fn move_players(
    time: Res<Time>,
    session: Res<Session>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &Transform), (With<camera::FollowCamera>, Without<Player>)>,
    covers: Query<(&Transform, &Cover), (With<Cover>, Without<Player>)>,
    mut players: Query<
        (
            &mut Transform,
            &PlayerIntent,
            &MovementSpeed,
            &GunslingerWeapon,
        ),
        With<Player>,
    >,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let rear_limit = cameras.single().ok().and_then(|(camera, transform)| {
        windows
            .single()
            .ok()
            .and_then(|window| camera::rear_limit(camera, transform, window))
    });
    for (mut transform, intent, speed, weapon) in &mut players {
        let speed_factor = if is_shooting(intent, weapon) {
            GUNSLINGER_PLAYER.shoot_move_factor
        } else {
            1.0
        };
        transform.translation += Vec3::new(intent.movement.x, 0.0, intent.movement.y)
            * speed.0
            * speed_factor
            * time.delta_secs();
        transform.translation.x = route::clamp_actor_x(
            transform.translation.x,
            transform.translation.z,
            GUNSLINGER_PLAYER.edge_radius,
        );
        for (cover_transform, cover) in &covers {
            enemy::separate_from_cover(
                &mut transform.translation,
                GUNSLINGER_PLAYER.cover_radius,
                cover_transform.translation,
                cover,
            );
        }
        transform.translation.x = route::clamp_actor_x(
            transform.translation.x,
            transform.translation.z,
            GUNSLINGER_PLAYER.edge_radius,
        );
        if let Some(limit) = rear_limit {
            transform.translation.z = transform.translation.z.min(limit);
        }
    }
}

pub fn face_players(
    session: Res<Session>,
    mut players: Query<(&mut Transform, &PlayerIntent), With<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (mut transform, intent) in &mut players {
        let direction = if intent.movement.length_squared() > 0.001 {
            Vec3::new(intent.movement.x, 0.0, intent.movement.y)
        } else {
            let mut direction = intent.aim - transform.translation;
            direction.y = 0.0;
            direction
        };
        if direction.length_squared() > 0.001 {
            transform.look_to(direction, Vec3::Y);
        }
    }
}

pub fn shoot(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &Transform,
            &PlayerIntent,
            &mut FireCooldown,
            &mut GunslingerWeapon,
        ),
        With<Player>,
    >,
) {
    if session.phase != Phase::Playing {
        return;
    }
    for (entity, transform, intent, mut cooldown, mut weapon) in &mut players {
        cooldown.0 = cooldown.0.saturating_sub(time.delta());
        if !intent.firing || cooldown.0 > Duration::ZERO || !weapon.can_fire() {
            weapon.advance_reload(time.delta());
            continue;
        }
        let mut direction = intent.aim - transform.translation;
        direction.y = 0.0;
        let direction = direction.normalize_or_zero();
        if direction == Vec3::ZERO {
            weapon.advance_reload(time.delta());
            continue;
        }
        let Some(gun) = weapon.fire_round() else {
            weapon.advance_reload(time.delta());
            continue;
        };
        cooldown.0 = Duration::from_millis(GUNSLINGER_WEAPON.fire_interval_ms);
        let side = if gun == 0 { -1.0 } else { 1.0 };
        let lateral =
            Vec3::new(direction.z, 0.0, -direction.x) * side * GUNSLINGER_WEAPON.muzzle_side;
        let muzzle = transform.translation + direction * GUNSLINGER_WEAPON.muzzle_forward + lateral;
        for (shot_direction, shot_muzzle) in paired_shots(direction, muzzle) {
            commands.spawn((
                Projectile {
                    direction: shot_direction,
                    speed: GUNSLINGER_WEAPON.projectile_speed,
                    radius: GUNSLINGER_WEAPON.projectile_radius,
                    previous: shot_muzzle,
                    spent: false,
                    faction: Faction::Player,
                    owner: entity,
                },
                Damage(GUNSLINGER_WEAPON.projectile_damage),
                Lifetime(GUNSLINGER_WEAPON.projectile_lifetime),
                Mesh3d(visuals.player_bullet_mesh.clone()),
                MeshMaterial3d(visuals.bullet_material.clone()),
                Transform::from_translation(shot_muzzle)
                    .with_rotation(Quat::from_rotation_y(
                        shot_direction.x.atan2(shot_direction.z),
                    ))
                    .with_scale(Vec3::splat(GUNSLINGER_WEAPON.projectile_visual_scale)),
            ));
        }
        audio::play_game(&mut commands, sounds.player_shot(), 0.34, 1.0);
    }
}

fn paired_shots(direction: Vec3, muzzle: Vec3) -> [(Vec3, Vec3); 2] {
    let lateral = Vec3::new(direction.z, 0.0, -direction.x);
    [-1.0, 1.0].map(|side| {
        (
            (direction + lateral * side * GUNSLINGER_WEAPON.pair_spread).normalize(),
            muzzle + lateral * side * GUNSLINGER_WEAPON.pair_muzzle_offset,
        )
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn purchased_levels_rebuild_stats_without_accumulated_deltas() {
        let build = PlayerProgression {
            health_level: 2,
            capacity_level: 3,
            reload_level: 4,
        };
        let expected = PlayerStats {
            max_health: 1400,
            capacity: 9,
            reload_wait_ms: 750,
        };
        assert_eq!(build.stats(), expected);
        assert_eq!(
            PlayerProgression {
                health_level: 100,
                capacity_level: 100,
                reload_level: 100
            }
            .stats(),
            PlayerStats {
                max_health: GUNSLINGER_PLAYER.max_health,
                capacity: GUNSLINGER_WEAPON.max_capacity,
                reload_wait_ms: GUNSLINGER_WEAPON.min_reload_wait_ms
            }
        );
        let mut stats = PlayerProgression::default().stats();
        let mut health = Health(600);
        let mut weapon = GunslingerWeapon::default();
        apply_progression(build, &mut stats, &mut health, &mut weapon, false);
        assert_eq!(stats, expected);
        assert_eq!(health.0, 1000);
        assert_eq!(weapon.capacity, 9);
        assert_eq!(weapon.reload_wait_ms, 750);
        apply_progression(build, &mut stats, &mut health, &mut weapon, false);
        assert_eq!(health.0, 1000);
        assert_eq!(weapon.rounds, 9);
        apply_progression(build, &mut stats, &mut health, &mut weapon, true);
        assert_eq!(health.0, 1400);
        assert_eq!(weapon.rounds, 9);
    }

    #[test]
    fn quickloader_upgrades_use_exact_millisecond_steps() {
        assert_eq!(
            (0..=6).map(reload_wait_ms_for).collect::<Vec<_>>(),
            vec![1050, 975, 900, 825, 750, 675, 600]
        );
        assert_eq!(reload_wait_ms_for(100), 600);
        let mut weapon = GunslingerWeapon::default();
        weapon.apply_stats(
            PlayerProgression {
                reload_level: 1,
                ..default()
            }
            .stats(),
        );
        assert_eq!(weapon.reload_wait_ms, 975);
        assert_eq!(weapon.reload_remaining, Duration::from_millis(1050));
    }

    #[test]
    fn drums_track_shots_reload_and_capacity() {
        let mut weapon = GunslingerWeapon::default();
        assert_eq!(
            (0..6)
                .map(|_| weapon.fire_round().unwrap())
                .collect::<Vec<_>>(),
            vec![0, 1, 0, 1, 0, 1]
        );
        assert_eq!(weapon.rounds, 0);
        assert!(
            weapon
                .drums
                .iter()
                .all(|drum| drum.chambers.iter().all(|&loaded| !loaded))
        );
        weapon.load_round();
        assert_eq!(weapon.rounds, 1);
        assert_eq!(weapon.fire_round(), Some(0));
        assert_eq!(weapon.rounds, 0);
        weapon.apply_stats(
            PlayerProgression {
                capacity_level: 1,
                ..default()
            }
            .stats(),
        );
        assert_eq!(weapon.capacity, 7);
        assert_eq!(weapon.rounds, 1);
        assert_eq!(weapon.drums[0].chambers.len(), 4);
        assert_eq!(weapon.drums[1].chambers.len(), 3);
        assert_eq!(weapon.drums[0].next, 3);
        assert!(weapon.drums[0].chambers[weapon.drums[0].next]);
        weapon.refill();
        assert_eq!(weapon.rounds, weapon.capacity);
        assert!(
            weapon
                .drums
                .iter()
                .all(|drum| drum.chambers.iter().all(|&loaded| loaded))
        );
    }

    #[test]
    fn quiet_partial_magazine_refills_before_the_next_shot() {
        let mut weapon = GunslingerWeapon::default();
        for _ in 0..5 {
            weapon.fire_round();
        }
        assert_eq!(weapon.rounds, 1);
        assert!(weapon.can_fire());
        weapon.advance_reload(Duration::from_millis(1049));
        assert_eq!(weapon.rounds, 1);
        assert!(weapon.can_fire());
        weapon.advance_reload(Duration::from_millis(1));
        assert!(weapon.recharging);
        assert!(!weapon.can_fire());
        weapon.advance_reload(Duration::from_millis(
            5 * GUNSLINGER_WEAPON.quickload_chamber_ms,
        ));
        assert_eq!(weapon.rounds, weapon.capacity);
        assert!(weapon.can_fire());
    }

    #[test]
    fn another_shot_restarts_the_quiet_reload_wait() {
        let mut weapon = GunslingerWeapon::default();
        weapon.fire_round();
        weapon.advance_reload(Duration::from_millis(500));
        weapon.fire_round();
        assert_eq!(weapon.reload_remaining, Duration::from_millis(1050));
        weapon.advance_reload(Duration::from_millis(800));
        assert_eq!(weapon.rounds, 4);
        assert!(!weapon.recharging);
        weapon.advance_reload(Duration::from_millis(250));
        assert!(weapon.recharging);
    }

    #[test]
    fn six_alternating_shots_trigger_locked_quickload() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, shoot);
        let player = app
            .world_mut()
            .spawn((
                Player,
                PlayerIntent {
                    aim: Vec3::new(0.0, 0.0, -10.0),
                    firing: true,
                    ..default()
                },
                FireCooldown(Duration::ZERO),
                GunslingerWeapon::default(),
                Transform::from_xyz(0.0, PLAYER_Y, 0.0),
            ))
            .id();
        for _ in 0..6 {
            app.world_mut().get_mut::<FireCooldown>(player).unwrap().0 = Duration::ZERO;
            app.update();
        }
        let weapon = app.world().get::<GunslingerWeapon>(player).unwrap();
        assert_eq!(weapon.rounds, 0);
        assert!(weapon.recharging);
        assert!(!weapon.can_fire());
        let mut shots: Vec<_> = app
            .world_mut()
            .query::<(&Projectile, &Transform)>()
            .iter(app.world())
            .filter(|(shot, _)| shot.faction == Faction::Player)
            .map(|(_, transform)| transform.translation.x)
            .collect();
        assert_eq!(shots.len(), 12);
        assert!(
            app.world_mut()
                .query::<(&Projectile, &Transform)>()
                .iter(app.world())
                .all(|(shot, transform)| {
                    (shot.speed - 34.0 * 4.0 / 3.0).abs() < 0.001
                        && (shot.radius - 0.17 * 2.0 / 3.0).abs() < 0.001
                        && transform.scale == Vec3::splat(2.0 / 3.0)
                })
        );
        assert!(
            app.world_mut()
                .query::<(&Projectile, &Damage)>()
                .iter(app.world())
                .all(|(_, damage)| damage.0 == 2)
        );
        shots.sort_by(f32::total_cmp);
        assert!(shots[0] < -0.3 && shots[11] > 0.3);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1100));
        app.update();
        assert_eq!(
            app.world().get::<GunslingerWeapon>(player).unwrap().rounds,
            0
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(10));
        app.update();
        assert_eq!(
            app.world().get::<GunslingerWeapon>(player).unwrap().rounds,
            1
        );
        assert!(
            !app.world()
                .get::<GunslingerWeapon>(player)
                .unwrap()
                .can_fire()
        );
        assert_eq!(
            app.world_mut()
                .query::<&Projectile>()
                .iter(app.world())
                .count(),
            12
        );
        app.world_mut()
            .get_mut::<PlayerIntent>(player)
            .unwrap()
            .firing = false;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.update();
        let weapon = app.world().get::<GunslingerWeapon>(player).unwrap();
        assert_eq!(weapon.rounds, weapon.capacity);
        assert!(weapon.can_fire());
    }

    #[test]
    fn separate_player_intents_fire_independently() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, shoot);
        let spawn = |world: &mut World, x: f32| {
            world
                .spawn((
                    Player,
                    PlayerIntent {
                        aim: Vec3::new(x, 0.0, -10.0),
                        firing: true,
                        ..default()
                    },
                    FireCooldown(Duration::ZERO),
                    GunslingerWeapon::default(),
                    Transform::from_xyz(x, PLAYER_Y, 0.0),
                ))
                .id()
        };
        let first = spawn(app.world_mut(), -2.0);
        let second = spawn(app.world_mut(), 2.0);
        app.update();
        let owners: Vec<_> = app
            .world_mut()
            .query::<&Projectile>()
            .iter(app.world())
            .map(|projectile| projectile.owner)
            .collect();
        assert_eq!(owners.len(), 4);
        assert!(owners.contains(&first));
        assert!(owners.contains(&second));
    }

    #[test]
    fn paired_shots_stay_within_one_metre_of_each_other_at_fifty_metres() {
        let shots = paired_shots(Vec3::NEG_Z, Vec3::ZERO);
        let first = shots[0].1 + shots[0].0 * 50.0;
        let second = shots[1].1 + shots[1].0 * 50.0;
        assert!(first.distance(second) < 1.0);
        assert!((first.x + second.x).abs() < 0.001);
    }
}
