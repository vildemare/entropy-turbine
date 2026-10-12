//! Cover sites, the phase queue, and the enemies that spawn from it.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::grenadier;
use super::rifle::Rifle;
use super::zap_bot::ZAP_SCHEDULE;
use super::{Cover, Drift, Enemy, EnemyKind, EnemyReward, EnemyState, Tactic};
use crate::combat::{Abilities, Faction, Health, HitCircle};
use crate::player::MovementSpeed;
use crate::session::{Phase, Session};
use crate::world::camera::{self, FollowCamera};
use crate::world::route;
use crate::world::visuals::Visuals;

const MAX_ENEMIES: usize = 90;
const COVER_WIDTH: f32 = 5.4;
/// Obstacles are built this far ahead of the camera center, about twice the old
/// lead, so a cow meets a barricade with room to turn.
const OBSTACLE_PLACE_AHEAD: f32 = 48.0;
const RUBBLE_SIZE: Vec3 = Vec3::new(1.3, 0.8, 1.4);
const FIELD_COVER_SIZE: Vec3 = Vec3::new(2.1, 1.0, 1.9);
pub const WAVES_PER_PHASE: u32 = 10;

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
    grenadier: bool,
}

#[derive(Resource)]
pub struct EncounterDirector {
    planned_waves: VecDeque<PlannedWave>,
    planned_checkpoint_z: f32,
    pub group_number: u32,
    pub phase_number: u32,
    pub checkpoint_z: Option<f32>,
    pub(crate) zap_timer: f32,
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
            zap_timer: ZAP_SCHEDULE.delay.as_secs_f32(),
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
        self.zap_timer = ZAP_SCHEDULE.delay.as_secs_f32();
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

    pub(crate) fn random(&mut self) -> f32 {
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
    grenadier: bool,
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
        grenadier: wave_has_grenadier(phase, wave),
    }
}

/// The last wave of the first phase always brings one grenade walker. Later
/// waves include one about one time in eight, from the phase and wave index
/// so the route's random stream stays put.
fn wave_has_grenadier(phase: u32, wave: u32) -> bool {
    if phase == 1 {
        return wave + 1 == WAVES_PER_PHASE;
    }
    let mixed = phase.wrapping_mul(0x045d_9f3b) ^ wave.wrapping_mul(0x119d_e1f3) ^ 0xa5a5_5a5a;
    mixed % 8 == 0
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
    while let Some(wave) = director.planned_waves.front().copied() {
        if wave.z < center_z - OBSTACLE_PLACE_AHEAD {
            break;
        }
        director.planned_waves.pop_front();
        place_wave(&mut director, &mut commands, &visuals, wave);
    }
}

fn place_wave(
    director: &mut EncounterDirector,
    commands: &mut Commands,
    visuals: &Visuals,
    wave: PlannedWave,
) {
    let PlannedWave {
        index,
        x,
        z,
        formation,
        field_x,
        plan,
    } = wave;
    commands.spawn((
        Abilities::solid(),
        Cover {
            half_width: COVER_WIDTH * 0.5,
            half_depth: 0.5,
            height: 1.1,
        },
        CoverSite {
            timer: 0.0,
            formation,
            count: plan.count,
            group_id: index,
            heavy_count: plan.heavy_count,
            spawned: false,
            reinforcement_planned: plan.reinforcements,
            reinforcements_sent: false,
            grenadier: plan.grenadier,
        },
        Mesh3d(visuals.barricade_mesh.clone()),
        MeshMaterial3d(visuals.barricade_material.clone()),
        Transform::from_xyz(route::centerline_x(z) + x, 0.55, z),
    ));
    for side in [-1.0, 1.0] {
        commands.spawn((
            Abilities::solid(),
            Cover {
                half_width: RUBBLE_SIZE.x * 0.5,
                half_depth: RUBBLE_SIZE.z * 0.5,
                height: RUBBLE_SIZE.y,
            },
            Mesh3d(visuals.rubble_mesh.clone()),
            MeshMaterial3d(visuals.rubble_material.clone()),
            Transform::from_xyz(
                route::centerline_x(z - 0.15) + x + side * 2.6,
                RUBBLE_SIZE.y * 0.5,
                z - 0.15,
            )
            .with_scale(RUBBLE_SIZE),
        ));
    }
    // Leave the middle lane open while giving the player places to break
    // incoming fire between barricades.
    for (field_x, offset) in field_x.into_iter().zip([9.0, -11.0]) {
        commands.spawn((
            Abilities::solid(),
            Cover {
                half_width: FIELD_COVER_SIZE.x * 0.5,
                half_depth: FIELD_COVER_SIZE.z * 0.5,
                height: FIELD_COVER_SIZE.y,
            },
            Mesh3d(visuals.rubble_mesh.clone()),
            MeshMaterial3d(visuals.rubble_material.clone()),
            Transform::from_xyz(
                route::centerline_x(z + offset) + field_x,
                FIELD_COVER_SIZE.y * 0.5,
                z + offset,
            )
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
    cameras: Query<(&Camera, &Transform), With<FollowCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };
    let front_z = camera::visible_front_z(camera, camera_transform, windows.iter().next());
    let mut live_count = enemies.iter().len();
    for (cover, mut site) in &mut sites {
        if site.spawned || cover.translation.z < front_z {
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
            let gunner = kind.definition();
            let rifle = gunner.rifle();
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
            let flank_x = route::clamp_actor_x(
                cover.translation.x + side * 4.3,
                cover.translation.z,
                gunner.radius(),
            );
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
                        radius: gunner.radius(),
                        height: gunner.height(),
                        cover_z: cover.translation.z,
                        slot_x,
                        flank_x,
                        hold_timer: 0.45 + director.random() * 0.45,
                        evade_direction: side,
                        sight_blocked: false,
                        drift: Drift::default(),
                    },
                    Rifle::from_spec(rifle, 0.2 + director.random() * 0.75),
                    Health(gunner.health()),
                    gunner.runtime_abilities(),
                    HitCircle {
                        radius: gunner.radius(),
                        faction: Faction::Enemy,
                        swell: true,
                    },
                    EnemyReward {
                        kind,
                        hit_player: false,
                    },
                    MovementSpeed(gunner.speed()),
                    Transform::from_xyz(
                        start_x + route::centerline_x(start_z)
                            - route::centerline_x(cover.translation.z),
                        gunner.height(),
                        start_z,
                    ),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            spawn_enemy_visual(&mut commands, &visuals, enemy_entity, kind);
        }
        if site.grenadier {
            let side = if director.random() < 0.5 { -1.0 } else { 1.0 };
            let z = cover.translation.z - 8.0;
            let walker = crate::character::grenadier();
            let x = route::clamp_actor_x(cover.translation.x + side * 6.5, z, walker.radius());
            let seed = site.group_id.wrapping_mul(0x9e37_79b9) | 1;
            grenadier::spawn(
                &mut commands,
                &visuals,
                Vec3::new(x, walker.height(), z),
                side,
                seed,
            );
        }
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
            let gunner = kind.definition();
            let rifle = gunner.rifle();
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let x = route::centerline_x(z) + center_x + (index as f32 - 1.0) * 2.0;
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
                        radius: gunner.radius(),
                        height: gunner.height(),
                        cover_z: z,
                        slot_x: x,
                        flank_x: x,
                        hold_timer: 0.0,
                        evade_direction: side,
                        sight_blocked: false,
                        drift: Drift::default(),
                    },
                    Rifle::from_spec(rifle, 0.35 + director.random() * 0.6),
                    Health(gunner.health()),
                    gunner.runtime_abilities(),
                    HitCircle {
                        radius: gunner.radius(),
                        faction: Faction::Enemy,
                        swell: true,
                    },
                    EnemyReward {
                        kind,
                        hit_player: false,
                    },
                    MovementSpeed(gunner.speed()),
                    Transform::from_xyz(x, gunner.height(), z),
                    GlobalTransform::default(),
                    Visibility::default(),
                ))
                .id();
            spawn_enemy_visual(&mut commands, &visuals, actor, kind);
        }
    }
}

fn spawn_enemy_visual(commands: &mut Commands, visuals: &Visuals, actor: Entity, kind: EnemyKind) {
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
    use super::*;

    use crate::ability::Struck;
    use crate::audio::SoundBank;
    use crate::combat::{self, Damage, Delivery, Faction, Material, Projectile};

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
        assert!(plans.last().unwrap().grenadier);
        assert_eq!(plans.iter().filter(|plan| plan.grenadier).count(), 1);
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
    fn later_waves_rarely_include_a_grenade_walker() {
        let mut included = 0;
        let mut total = 0;
        for phase in 2..8 {
            for wave in 0..WAVES_PER_PHASE {
                total += 1;
                if wave_has_grenadier(phase, wave) {
                    included += 1;
                }
            }
        }
        assert!(included > 0, "a later phase should sometimes bring one");
        assert!(
            included * 4 < total,
            "included {included} of {total}, which is too often"
        );
    }

    #[test]
    fn a_wave_more_than_forty_eight_metres_ahead_stays_unbuilt() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(EncounterDirector::default())
            .add_systems(Update, place_covers);
        app.world_mut().spawn((
            FollowCamera,
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, 50.0),
        ));
        app.update();
        assert_eq!(
            app.world()
                .resource::<EncounterDirector>()
                .planned_waves
                .len(),
            WAVES_PER_PHASE as usize
        );
        assert_eq!(
            app.world_mut()
                .query::<&CoverSite>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn defenders_spawn_when_the_barricade_reaches_the_top_of_the_view() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .insert_resource(Visuals::default())
            .insert_resource(EncounterDirector::default())
            .add_systems(Update, spawn_groups);
        app.world_mut().spawn((
            FollowCamera,
            Camera::default(),
            Transform::from_xyz(0.0, camera::CAMERA_HEIGHT, camera::CAMERA_BACK_OFFSET),
        ));
        let waiting = app
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
                    count: 2,
                    group_id: 1,
                    heavy_count: 0,
                    spawned: false,
                    reinforcement_planned: false,
                    reinforcements_sent: false,
                    grenadier: false,
                },
                Transform::from_xyz(0.0, 0.55, -40.0),
            ))
            .id();
        app.update();
        assert!(!app.world().get::<CoverSite>(waiting).unwrap().spawned);
        assert_eq!(
            app.world_mut().query::<&Enemy>().iter(app.world()).count(),
            0
        );

        app.world_mut()
            .get_mut::<Transform>(waiting)
            .unwrap()
            .translation
            .z = -20.0;
        app.update();
        assert!(app.world().get::<CoverSite>(waiting).unwrap().spawned);
        assert_eq!(
            app.world_mut().query::<&Enemy>().iter(app.world()).count(),
            2
        );
    }

    #[test]
    fn visible_barricade_end_blocks_stop_shots_outside_the_center_wall() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(Visuals::default())
            .insert_resource(SoundBank::default())
            .insert_resource(EncounterDirector::default())
            .add_message::<Struck>()
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
        assert_eq!(
            (center_x, center_z),
            (route::centerline_x(planned.z) + planned.x, planned.z)
        );
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
                    speed: 34.0,
                    radius: 0.17,
                    previous: Vec3::new(x, 0.75, center_z + 3.0),
                    spent: false,
                    faction: Faction::Player,
                    material: Material::Plasma,
                    delivery: Delivery::Energy,
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
                    speed: 34.0,
                    radius: 0.17,
                    previous: field_position + Vec3::new(0.0, 0.25, 3.0),
                    spent: false,
                    faction: Faction::Player,
                    material: Material::Plasma,
                    delivery: Delivery::Energy,
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
                    grenadier: false,
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
                    evade_direction: 1.0,
                    sight_blocked: false,
                    drift: Drift::default(),
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
