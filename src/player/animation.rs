//! Loads the player model and drives its movement and aim poses.

use std::time::Duration;

use bevy::prelude::*;

use crate::player::{self, GunslingerWeapon, Player, PlayerIntent};
use crate::session::{Phase, Session};
use crate::tuning::GUNSLINGER_PLAYER;

const MODEL_PATH: &str = "models/toon_soldier.gltf";

#[derive(Resource, Clone)]
pub struct CharacterAsset {
    pub gltf: Handle<Gltf>,
    pub scene: Handle<WorldAsset>,
}

impl CharacterAsset {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            gltf: asset_server.load(MODEL_PATH),
            scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(MODEL_PATH)),
        }
    }
}

#[derive(Component)]
pub struct CharacterModel;

#[derive(Resource)]
pub struct CharacterAnimations {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    run: AnimationNodeIndex,
    idle_shoot: AnimationNodeIndex,
    run_shoot: AnimationNodeIndex,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Motion {
    Idle,
    Run,
    IdleShoot,
    RunShoot,
}

#[derive(Component)]
pub(crate) struct AnimationDriver {
    actor: Entity,
    current: Motion,
}

#[derive(Component)]
pub(crate) struct CharacterBound;

#[derive(Component)]
pub(crate) struct UpperBodyAim {
    actor: Entity,
}

pub fn load_animations(
    mut commands: Commands,
    character: Res<CharacterAsset>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    loaded: Option<Res<CharacterAnimations>>,
) {
    if loaded.is_some() {
        return;
    }
    let Some(gltf) = gltfs.get(&character.gltf) else {
        return;
    };
    let (Some(idle), Some(run), Some(idle_shoot), Some(run_shoot)) = (
        gltf.named_animations.get("Idle"),
        gltf.named_animations.get("Run"),
        gltf.named_animations.get("Idle_Shoot"),
        gltf.named_animations.get("Run_Shoot"),
    ) else {
        return;
    };
    let (graph, indices) = AnimationGraph::from_clips([
        idle.clone(),
        run.clone(),
        idle_shoot.clone(),
        run_shoot.clone(),
    ]);
    info!("Toon Soldier loaded with movement and shooting animations");
    commands.insert_resource(CharacterAnimations {
        graph: graphs.add(graph),
        idle: indices[0],
        run: indices[1],
        idle_shoot: indices[2],
        run_shoot: indices[3],
    });
}

pub fn bind_animations(
    mut commands: Commands,
    animations: Option<Res<CharacterAnimations>>,
    roots: Query<(Entity, &ChildOf), (With<CharacterModel>, Without<CharacterBound>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut players: Query<(&mut AnimationPlayer, Option<&AnimationGraphHandle>)>,
) {
    let Some(animations) = animations else {
        return;
    };
    for (root, parent) in &roots {
        let mut animation_found = false;
        let mut torso_found = false;
        for descendant in children.iter_descendants(root) {
            if let Ok(name) = names.get(descendant) {
                if name.as_str() == "Torso" {
                    commands.entity(descendant).insert(UpperBodyAim {
                        actor: parent.parent(),
                    });
                    torso_found = true;
                }
            }
            if let Ok((mut player, graph_handle)) = players.get_mut(descendant) {
                animation_found = true;
                if graph_handle.is_none() {
                    let mut transitions = AnimationTransitions::new();
                    transitions
                        .play(&mut player, animations.idle, Duration::ZERO)
                        .repeat();
                    commands.entity(descendant).insert((
                        AnimationGraphHandle(animations.graph.clone()),
                        transitions,
                        AnimationDriver {
                            actor: parent.parent(),
                            current: Motion::Idle,
                        },
                    ));
                }
            }
        }
        if animation_found && torso_found {
            commands.entity(root).insert(CharacterBound);
            info!("Player character animation and torso aiming ready");
        }
    }
}

pub fn animate_characters(
    animations: Option<Res<CharacterAnimations>>,
    actors: Query<(&PlayerIntent, &GunslingerWeapon), With<Player>>,
    mut players: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut AnimationDriver,
    )>,
) {
    let Some(animations) = animations else {
        return;
    };
    for (mut player, mut transitions, mut driver) in &mut players {
        let Ok((intent, weapon)) = actors.get(driver.actor) else {
            continue;
        };
        let shooting = player::is_shooting(intent, weapon);
        let desired = match (intent.movement.length_squared() > 0.001, shooting) {
            (false, false) => Motion::Idle,
            (true, false) => Motion::Run,
            (false, true) => Motion::IdleShoot,
            (true, true) => Motion::RunShoot,
        };
        if desired != driver.current {
            driver.current = desired;
            let clip = match desired {
                Motion::Idle => animations.idle,
                Motion::Run => animations.run,
                Motion::IdleShoot => animations.idle_shoot,
                Motion::RunShoot => animations.run_shoot,
            };
            transitions
                .play(&mut player, clip, Duration::from_millis(140))
                .repeat();
        }
        let speed_factor = if shooting {
            GUNSLINGER_PLAYER.shoot_move_factor
        } else {
            1.0
        };
        for clip in [
            animations.idle,
            animations.run,
            animations.idle_shoot,
            animations.run_shoot,
        ] {
            if let Some(active) = player.animation_mut(clip) {
                active.set_speed(speed_factor);
            }
        }
    }
}

pub fn aim_upper_body(
    session: Res<Session>,
    mut actors: Query<
        (&PlayerIntent, &GunslingerWeapon, &mut Transform),
        (With<Player>, Without<UpperBodyAim>),
    >,
    mut torsos: Query<(&UpperBodyAim, &mut Transform), Without<Player>>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    const MAX_TORSO_TWIST: f32 = std::f32::consts::FRAC_PI_2;
    for (aim, mut torso) in &mut torsos {
        let Ok((intent, weapon, mut actor)) = actors.get_mut(aim.actor) else {
            continue;
        };
        if !player::is_shooting(intent, weapon) {
            continue;
        }
        let mut target = intent.aim - actor.translation;
        target.y = 0.0;
        let target = target.normalize_or_zero();
        if target == Vec3::ZERO {
            continue;
        }
        let forward = actor.rotation * Vec3::NEG_Z;
        let angle = forward.cross(target).y.atan2(forward.dot(target));
        let twist = angle.clamp(-MAX_TORSO_TWIST, MAX_TORSO_TWIST);
        actor.rotation = Quat::from_rotation_y(angle - twist) * actor.rotation;
        torso.rotation *= Quat::from_rotation_y(twist);
    }
}
