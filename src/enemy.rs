use std::collections::VecDeque;

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    camera::{self, FollowCamera},
    combat::{self, Damage, Faction, Health, Lifetime, Projectile},
    game::{Phase, Session, Visuals},
    player::{MovementSpeed, Player},
    sound::{self, SoundBank},
    street::{self, Boulder},
};

const MAX_ENEMIES: usize = 90;
const COVER_WIDTH: f32 = 5.4;
const RUBBLE_SIZE: Vec3 = Vec3::new(1.3, 0.8, 1.4);
const FIELD_COVER_SIZE: Vec3 = Vec3::new(2.1, 1.0, 1.9);
const RETREAT_SPEED: f32 = 6.0;
const RETREAT_TIMEOUT: f32 = 3.5;
const COVER_ENTRY_SPEED: f32 = 1.5;
const COVER_EXIT_SPEED: f32 = 1.5;
const SIGHT_RANGE: f32 = 33.0;
const BURST_SHOTS: u8 = 3;
pub const WAVES_PER_PHASE: u32 = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    Scout,
    Trooper,
    Heavy,
}

fn shot_damage(kind: EnemyKind) -> u32 {
    match kind {
        EnemyKind::Scout => 150,
        EnemyKind::Trooper => 200,
        EnemyKind::Heavy => 300,
    }
}

#[derive(Component)]
pub struct EnemyReward {
    pub kind: EnemyKind,
    pub hit_player: bool,
}

impl EnemyReward {
    pub fn points(&self) -> u32 {
        let base = match self.kind {
            EnemyKind::Scout => 1,
            EnemyKind::Trooper => 2,
            EnemyKind::Heavy => 4,
        };
        base + u32::from(self.hit_player)
    }
}

#[derive(Clone, Copy)]
pub enum Tactic {
    Straight,
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyState {
    Entering,
    Holding,
    Flanking,
    Exiting,
    Advancing,
    Retreating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackPhase {
    Burst,
    Reloading,
}

#[derive(Component)]
pub struct Enemy {
    pub group_id: u32,
    pub engaged: bool,
    pub state: EnemyState,
    pub tactic: Tactic,
    pub radius: f32,
    pub height: f32,
    pub cover_z: f32,
    pub slot_x: f32,
    pub flank_x: f32,
    pub hold_timer: f32,
    pub fire_timer: f32,
    pub attack_phase: AttackPhase,
    pub shots_left: u8,
    pub evade_direction: f32,
    pub sight_blocked: bool,
    pub reload_duration: f32,
}

#[derive(Component)]
pub struct Cover {
    pub half_width: f32,
    pub half_depth: f32,
    pub height: f32,
}

#[derive(Component)]
pub struct Retreating {
    side: f32,
    rear_z: f32,
    remaining: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Formation {
    Row,
    File,
}

#[derive(Component)]
pub struct CoverSite {
    timer: f32,
    formation: Formation,
    count: usize,
    group_id: u32,
    heavy_count: usize,
    spawned: bool,
    reinforcement_planned: bool,
    reinforcements_sent: bool,
}

#[derive(Resource)]
pub struct EncounterDirector {
    planned_waves: VecDeque<PlannedWave>,
    planned_checkpoint_z: f32,
    pub group_number: u32,
    pub phase_number: u32,
    pub checkpoint_z: Option<f32>,
    random_state: u32,
}

impl Default for EncounterDirector {
    fn default() -> Self {
        let mut director = Self {
            planned_waves: VecDeque::new(),
            planned_checkpoint_z: 0.0,
            group_number: 0,
            phase_number: 1,
            checkpoint_z: None,
            random_state: 0x8b45_73cf,
        };
        director.prepare_phase(-23.0);
        director
    }
}

impl EncounterDirector {
    pub fn next_phase(&mut self) {
        let checkpoint_z = self
            .checkpoint_z
            .take()
            .unwrap_or(self.planned_checkpoint_z);
        self.phase_number += 1;
        self.group_number = 0;
        self.prepare_phase(checkpoint_z - 25.0);
    }

    fn prepare_phase(&mut self, start_z: f32) {
        self.planned_waves.clear();
        let mut z = start_z;
        for index in 0..WAVES_PER_PHASE {
            let plan = wave_plan(self.phase_number, index);
            let x = (self.random() * 2.0 - 1.0) * 5.0;
            let field_x = [-(5.5 + self.random() * 2.0), 5.5 + self.random() * 2.0];
            self.planned_waves.push_back(PlannedWave {
                index,
                x,
                z,
                formation: if index.is_multiple_of(2) {
                    Formation::Row
                } else {
                    Formation::File
                },
                field_x,
                plan,
            });
            z -= 31.0 + self.random() * 7.0;
        }
        self.planned_checkpoint_z = self.planned_waves.back().unwrap().z - 25.0;
    }

    fn random(&mut self) -> f32 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 17;
        self.random_state ^= self.random_state << 5;
        self.random_state as f32 / u32::MAX as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WavePlan {
    count: usize,
    heavy_count: usize,
    reinforcements: bool,
}

#[derive(Clone, Copy, Debug)]
struct PlannedWave {
    index: u32,
    x: f32,
    z: f32,
    formation: Formation,
    field_x: [f32; 2],
    plan: WavePlan,
}

fn wave_plan(phase: u32, wave: u32) -> WavePlan {
    let tier = phase.saturating_sub(1).min(3) as usize;
    WavePlan {
        count: (3 + (wave / 3) as usize + tier).min(7),
        heavy_count: usize::from(wave >= 4) + usize::from(wave >= 8 && phase >= 2),
        reinforcements: wave == 3 || wave == 6 || wave == 9 || (phase >= 2 && wave == 7),
    }
}

pub fn place_covers(
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut director: ResMut<EncounterDirector>,
    mut commands: Commands,
    cameras: Query<&Transform, With<FollowCamera>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let center_z = camera.translation.z - camera::CAMERA_BACK_OFFSET;
    let Some(wave) = director.planned_waves.front().copied() else {
        return;
    };
    if wave.z < center_z - 24.0 {
        return;
    }
    director.planned_waves.pop_front();
    let PlannedWave {
        index,
        x,
        z,
        formation,
        field_x,
        plan,
    } = wave;
    commands.spawn((
        Cover {
            half_width: COVER_WIDTH * 0.5,
            half_depth: 0.5,
            height: 1.1,
        },
        CoverSite {
            timer: 1.2,
            formation,
            count: plan.count,
            group_id: index,
            heavy_count: plan.heavy_count,
            spawned: false,
            reinforcement_planned: plan.reinforcements,
            reinforcements_sent: false,
        },
        Mesh3d(visuals.barricade_mesh.clone()),
        MeshMaterial3d(visuals.barricade_material.clone()),
        Transform::from_xyz(x, 0.55, z),
    ));
    for side in [-1.0, 1.0] {
        commands.spawn((
            Cover {
                half_width: RUBBLE_SIZE.x * 0.5,
                half_depth: RUBBLE_SIZE.z * 0.5,
                height: RUBBLE_SIZE.y,
            },
            Mesh3d(visuals.rubble_mesh.clone()),
            MeshMaterial3d(visuals.rubble_material.clone()),
            Transform::from_xyz(x + side * 2.6, RUBBLE_SIZE.y * 0.5, z - 0.15)
                .with_scale(RUBBLE_SIZE),
        ));
    }
    // Leave the middle lane open while giving the player places to break
    // incoming fire between barricades.
    for (field_x, offset) in field_x.into_iter().zip([9.0, -11.0]) {
        commands.spawn((
            Cover {
                half_width: FIELD_COVER_SIZE.x * 0.5,
                half_depth: FIELD_COVER_SIZE.z * 0.5,
                height: FIELD_COVER_SIZE.y,
            },
            Mesh3d(visuals.rubble_mesh.clone()),
            MeshMaterial3d(visuals.rubble_material.clone()),
            Transform::from_xyz(field_x, FIELD_COVER_SIZE.y * 0.5, z + offset)
                .with_scale(FIELD_COVER_SIZE),
        ));
    }
    director.group_number += 1;
    if director.group_number == WAVES_PER_PHASE {
        director.checkpoint_z = Some(director.planned_checkpoint_z);
    }
}

pub fn spawn_groups(
    time: Res<Time>,
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut director: ResMut<EncounterDirector>,
    mut commands: Commands,
    mut sites: Query<(&Transform, &mut CoverSite), With<Cover>>,
    enemies: Query<Entity, With<Enemy>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let mut live_count = enemies.iter().len();
    for (cover, mut site) in &mut sites {
        if site.spawned {
            continue;
        }
        site.timer -= time.delta_secs();
        if site.timer > 0.0 || live_count + site.count > MAX_ENEMIES {
            continue;
        }
        site.spawned = true;
        live_count += site.count;
        let center = (site.count as f32 - 1.0) * 0.5;
        for index in 0..site.count {
            let slot = index as f32 - center;
            let kind = if index >= site.count - site.heavy_count {
                EnemyKind::Heavy
            } else if index % 3 == 0 {
                EnemyKind::Scout
            } else {
                EnemyKind::Trooper
            };
            let (hit_points, radius, height, speed, reload_duration) = enemy_stats(kind);
            let slot_x = cover.translation.x + slot * 1.45;
            let start_x = match site.formation {
                Formation::Row => slot_x,
                Formation::File => cover.translation.x,
            };
            let start_z = cover.translation.z
                - 4.5
                - if matches!(site.formation, Formation::File) {
                    index as f32 * 1.3
                } else {
                    0.0
                };
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let flank_x = street::clamp_actor_x(cover.translation.x + side * 4.3, radius);
            let enemy_entity = commands
                .spawn((
                    Enemy {
                        group_id: site.group_id,
                        engaged: false,
                        state: EnemyState::Entering,
                        tactic: if index % 2 == 0 {
                            Tactic::Straight
                        } else {
                            Tactic::Direct
                        },
                        radius,
                        height,
                        cover_z: cover.translation.z,
                        slot_x,
                        flank_x,
                        hold_timer: 0.45 + director.random() * 0.45,
                        fire_timer: 0.2 + director.random() * 0.75,
                        attack_phase: AttackPhase::Burst,
                        shots_left: BURST_SHOTS,
                        evade_direction: side,
                        sight_blocked: false,
                        reload_duration,
                    },
                    Health(hit_points),
                    EnemyReward {
                        kind,
                        hit_player: false,
                    },
                    MovementSpeed(speed),
                    Transform::from_xyz(start_x, height, start_z),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            spawn_enemy_visual(&mut commands, &visuals, enemy_entity, kind);
        }
    }
}

fn enemy_stats(kind: EnemyKind) -> (u32, f32, f32, f32, f32) {
    match kind {
        EnemyKind::Scout => (1, 0.44, 0.54, 3.7, 1.15),
        EnemyKind::Trooper => (2, 0.56, 0.68, 3.0, 1.55),
        EnemyKind::Heavy => (5, 0.73, 0.83, 2.1, 1.95),
    }
}

pub fn spawn_reinforcements(
    session: Res<Session>,
    visuals: Res<Visuals>,
    mut director: ResMut<EncounterDirector>,
    mut commands: Commands,
    cameras: Query<&Transform, With<FollowCamera>>,
    mut sites: Query<(&Transform, &mut CoverSite), With<Cover>>,
    enemies: Query<&Enemy>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let mut live_count = enemies.iter().len();
    let camera_center = camera.translation.z - camera::CAMERA_BACK_OFFSET;
    for (cover, mut site) in &mut sites {
        if !site.spawned || site.reinforcements_sent || !site.reinforcement_planned {
            continue;
        }
        if !enemies.iter().any(|enemy| {
            enemy.group_id == site.group_id
                && enemy.engaged
                && enemy.state != EnemyState::Retreating
        }) {
            continue;
        }
        if live_count + 3 > MAX_ENEMIES {
            continue;
        }
        site.reinforcements_sent = true;
        live_count += 3;
        let z = (cover.translation.z - 13.0).min(camera_center - 27.0);
        let center_x = (director.random() * 2.0 - 1.0) * 3.0;
        for index in 0..3 {
            let kind = if index == 1 {
                EnemyKind::Trooper
            } else {
                EnemyKind::Scout
            };
            let (hit_points, radius, height, speed, reload_duration) = enemy_stats(kind);
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let x = center_x + (index as f32 - 1.0) * 2.0;
            let actor = commands
                .spawn((
                    Enemy {
                        group_id: site.group_id | 0x8000_0000,
                        engaged: false,
                        state: EnemyState::Advancing,
                        tactic: if index == 1 {
                            Tactic::Direct
                        } else {
                            Tactic::Straight
                        },
                        radius,
                        height,
                        cover_z: z,
                        slot_x: x,
                        flank_x: x,
                        hold_timer: 0.0,
                        fire_timer: 0.35 + director.random() * 0.6,
                        attack_phase: AttackPhase::Burst,
                        shots_left: BURST_SHOTS,
                        evade_direction: side,
                        sight_blocked: false,
                        reload_duration,
                    },
                    Health(hit_points),
                    EnemyReward {
                        kind,
                        hit_player: false,
                    },
                    MovementSpeed(speed),
                    Transform::from_xyz(x, height, z),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            spawn_enemy_visual(&mut commands, &visuals, actor, kind);
        }
    }
}

fn spawn_enemy_visual(commands: &mut Commands, visuals: &Visuals, actor: Entity, kind: EnemyKind) {
    let size = match kind {
        EnemyKind::Scout => 0.80,
        EnemyKind::Trooper => 1.0,
        EnemyKind::Heavy => 1.22,
    };
    let coat = match kind {
        EnemyKind::Scout => &visuals.scout_material,
        EnemyKind::Trooper => &visuals.trooper_material,
        EnemyKind::Heavy => &visuals.heavy_material,
    };
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
                    speed.0 * COVER_ENTRY_SPEED * dt,
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
                    speed.0 * COVER_EXIT_SPEED * dt,
                );
                if transform.translation.distance_squared(target) < 0.18 * 0.18 {
                    enemy.state = EnemyState::Exiting;
                }
            }
            EnemyState::Exiting => {
                transform.translation.z += speed.0 * COVER_EXIT_SPEED * dt;
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
                    if transform.translation.x.abs() > street::ROAD_HALF_WIDTH - enemy.radius - 0.5
                    {
                        enemy.evade_direction *= -1.0;
                    }
                } else if distance > SIGHT_RANGE
                    || distance > 5.5 && enemy.fire_timer > 0.0 && enemy.shots_left == BURST_SHOTS
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
                    transform.translation.x += retreat.side * RETREAT_SPEED * dt;
                    transform.translation.z = transform.translation.z.min(retreat.rear_z);
                    transform.rotation =
                        Quat::from_rotation_y(retreat.side * std::f32::consts::FRAC_PI_2);
                }
            }
        }
        transform.translation.x = street::clamp_actor_x(transform.translation.x, enemy.radius);
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
            a.translation.x = street::clamp_actor_x(a.translation.x + shift.x, enemy_a.radius);
            a.translation.z += shift.y;
            b.translation.x = street::clamp_actor_x(b.translation.x - shift.x, enemy_b.radius);
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
        transform.translation.x = street::clamp_actor_x(transform.translation.x, enemy.radius);
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
                enemy.shots_left = BURST_SHOTS;
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
            || origin.distance_squared(destination) > SIGHT_RANGE * SIGHT_RANGE
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
            enemy.shots_left = BURST_SHOTS;
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
        commands.spawn((
            Projectile {
                direction,
                previous: start,
                spent: false,
                faction: Faction::Enemy,
                owner: entity,
            },
            Damage(shot_damage(
                reward
                    .map(|reward| reward.kind)
                    .unwrap_or(EnemyKind::Trooper),
            )),
            Lifetime(2.5),
            Mesh3d(visuals.bullet_mesh.clone()),
            MeshMaterial3d(visuals.enemy_bullet_material.clone()),
            Transform::from_translation(start),
        ));
        let volume = (0.26 - origin.distance(destination) * 0.004).clamp(0.08, 0.26);
        let speed = 0.94 + (BURST_SHOTS - enemy.shots_left) as f32 * 0.04;
        sound::play_game(&mut commands, sounds.enemy_shot(), volume, speed);
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
            if transform.translation.x.abs() >= street::ROAD_HALF_WIDTH - enemy.radius - 0.15
                || retreat.remaining <= 0.0
            {
                commands.entity(entity).despawn();
            }
            continue;
        }
        if transform.translation.z > enemy_cutoff {
            let side = if transform.translation.x > 0.0 {
                1.0
            } else if transform.translation.x < 0.0 {
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
                remaining: RETREAT_TIMEOUT,
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
    fn phase_one_has_ten_escalating_waves_and_three_reinforcements() {
        let plans: Vec<_> = (0..WAVES_PER_PHASE)
            .map(|wave| wave_plan(1, wave))
            .collect();
        assert_eq!(
            plans.iter().map(|plan| plan.count).collect::<Vec<_>>(),
            vec![3, 3, 3, 4, 4, 4, 5, 5, 5, 6]
        );
        assert_eq!(plans.iter().filter(|plan| plan.reinforcements).count(), 3);
        assert_eq!(plans.iter().map(|plan| plan.heavy_count).sum::<usize>(), 6);
        assert!(wave_plan(2, 9).count > wave_plan(1, 9).count);
        assert!(wave_plan(2, 9).heavy_count > wave_plan(1, 9).heavy_count);
        assert_eq!(
            EnemyReward {
                kind: EnemyKind::Scout,
                hit_player: false
            }
            .points(),
            1
        );
        assert_eq!(
            EnemyReward {
                kind: EnemyKind::Heavy,
                hit_player: true
            }
            .points(),
            5
        );
        let mut director = EncounterDirector::default();
        assert_eq!(director.planned_waves.len(), WAVES_PER_PHASE as usize);
        assert_eq!(director.planned_waves.front().unwrap().z, -23.0);
        assert!(
            director
                .planned_waves
                .iter()
                .zip(director.planned_waves.iter().skip(1))
                .all(|(a, b)| a.z > b.z && a.index + 1 == b.index)
        );
        assert_eq!(
            director
                .planned_waves
                .iter()
                .filter(|wave| wave.plan.reinforcements)
                .count(),
            3
        );
        assert_eq!(
            director.planned_checkpoint_z,
            director.planned_waves.back().unwrap().z - 25.0
        );
        director.group_number = WAVES_PER_PHASE;
        director.checkpoint_z = Some(-380.0);
        director.next_phase();
        assert_eq!(director.phase_number, 2);
        assert_eq!(director.group_number, 0);
        assert_eq!(director.planned_waves.front().unwrap().z, -405.0);
        assert_eq!(director.planned_waves.len(), WAVES_PER_PHASE as usize);
        assert_eq!(director.checkpoint_z, None);
    }

    #[test]
    fn enemy_shot_damage_uses_player_health_scale() {
        assert_eq!(shot_damage(EnemyKind::Scout), 150);
        assert_eq!(shot_damage(EnemyKind::Trooper), 200);
        assert_eq!(shot_damage(EnemyKind::Heavy), 300);
    }

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
            .x = street::ROAD_HALF_WIDTH - 0.56;
        app.update();
        assert!(app.world().get_entity(enemy).is_err());
    }

    #[test]
    fn visible_barricade_end_blocks_stop_shots_outside_the_center_wall() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .insert_resource(EncounterDirector::default())
            .add_systems(Update, (place_covers, combat::resolve_hits).chain());
        let planned = *app
            .world()
            .resource::<EncounterDirector>()
            .planned_waves
            .front()
            .unwrap();
        app.world_mut().spawn((
            FollowCamera,
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        app.update();
        let field_cover_count = app
            .world_mut()
            .query_filtered::<(&Cover, &Transform), Without<CoverSite>>()
            .iter(app.world())
            .filter(|(_, transform)| transform.scale == FIELD_COVER_SIZE)
            .count();
        assert_eq!(field_cover_count, 2);
        let (center_x, center_z) = {
            let world = app.world_mut();
            let mut sites = world.query_filtered::<&Transform, With<CoverSite>>();
            let center = sites.single(world).unwrap().translation;
            (center.x, center.z)
        };
        assert_eq!((center_x, center_z), (planned.x, planned.z));
        assert_eq!(
            app.world()
                .resource::<EncounterDirector>()
                .planned_waves
                .len(),
            9
        );
        let x = center_x + 3.1;
        let shot = app
            .world_mut()
            .spawn((
                Projectile {
                    direction: Vec3::NEG_Z,
                    previous: Vec3::new(x, 0.75, center_z + 3.0),
                    spent: false,
                    faction: Faction::Player,
                    owner: Entity::PLACEHOLDER,
                },
                Damage(1),
                Transform::from_xyz(x, 0.75, center_z - 3.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(shot).unwrap().spent);

        let field_position = {
            let world = app.world_mut();
            let mut covers =
                world.query_filtered::<&Transform, (With<Cover>, Without<CoverSite>)>();
            covers
                .iter(world)
                .find(|transform| transform.scale == FIELD_COVER_SIZE)
                .unwrap()
                .translation
        };
        let field_shot = app
            .world_mut()
            .spawn((
                Projectile {
                    direction: Vec3::NEG_Z,
                    previous: field_position + Vec3::new(0.0, 0.25, 3.0),
                    spent: false,
                    faction: Faction::Player,
                    owner: Entity::PLACEHOLDER,
                },
                Damage(1),
                Transform::from_translation(field_position + Vec3::new(0.0, 0.25, -3.0)),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Projectile>(field_shot).unwrap().spent);
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
                    remaining: RETREAT_TIMEOUT,
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
            shots_left: BURST_SHOTS,
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
        assert_eq!(flanker_state.shots_left, BURST_SHOTS - 1);
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
                    shots_left: BURST_SHOTS,
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

    #[test]
    fn planned_reinforcements_arrive_once_in_a_row_after_engagement() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(EncounterDirector::default())
            .add_systems(Update, spawn_reinforcements);
        app.world_mut().spawn((
            FollowCamera,
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        let cover = app
            .world_mut()
            .spawn((
                Cover {
                    half_width: 2.7,
                    half_depth: 0.5,
                    height: 1.1,
                },
                CoverSite {
                    timer: 0.0,
                    formation: Formation::Row,
                    count: 3,
                    group_id: 7,
                    heavy_count: 0,
                    spawned: true,
                    reinforcement_planned: true,
                    reinforcements_sent: false,
                },
                Transform::from_xyz(0.0, 0.55, -23.0),
            ))
            .id();
        let first = app
            .world_mut()
            .spawn((
                Enemy {
                    group_id: 7,
                    engaged: false,
                    state: EnemyState::Advancing,
                    tactic: Tactic::Direct,
                    radius: 0.56,
                    height: 0.68,
                    cover_z: -23.0,
                    slot_x: 0.0,
                    flank_x: 0.0,
                    hold_timer: 0.0,
                    fire_timer: 0.0,
                    attack_phase: AttackPhase::Burst,
                    shots_left: BURST_SHOTS,
                    evade_direction: 1.0,
                    sight_blocked: false,
                    reload_duration: 1.5,
                },
                Transform::from_xyz(0.0, 0.68, -20.0),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world_mut().query::<&Enemy>().iter(app.world()).count(),
            1
        );

        app.world_mut().get_mut::<Enemy>(first).unwrap().engaged = true;
        app.update();
        let row: Vec<_> = app
            .world_mut()
            .query::<(&Enemy, &Transform)>()
            .iter(app.world())
            .filter(|(enemy, _)| enemy.group_id != 7)
            .map(|(enemy, transform)| (enemy.state, transform.translation.z))
            .collect();
        assert_eq!(row.len(), 3);
        assert!(
            row.iter()
                .all(|(state, z)| *state == EnemyState::Advancing && *z == row[0].1)
        );
        assert!(
            app.world()
                .get::<CoverSite>(cover)
                .unwrap()
                .reinforcements_sent
        );
        app.update();
        assert_eq!(
            app.world_mut().query::<&Enemy>().iter(app.world()).count(),
            4
        );
    }
}
