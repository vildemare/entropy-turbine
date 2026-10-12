//! The gunner's rifle. Behavior reports an [`RifleAim`]. This module owns the
//! burst, the waits, and the round that leaves the muzzle. A [`RoundFired`]
//! tells the behavior that a round left and whether the burst is spent.

use bevy::prelude::*;

use crate::audio::{self, SoundBank};
use crate::character::RifleSpec;
use crate::combat::{Damage, Delivery, Faction, Lifetime, Material, Projectile};
use crate::world::visuals::Visuals;

/// Pause after a reload before the next burst.
const AFTER_RELOAD: f32 = 0.1;
/// Cap on the wait while a target is past the rifle's sight.
const FAR_WAIT: f32 = 0.3;
/// Cap on the wait while something stands in the line.
const BLOCKED_WAIT: f32 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackPhase {
    Burst,
    Reloading,
}

/// What the behavior can see for one gunner this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// No player to measure.
    Unseen,
    /// A player is past the rifle's sight.
    Far,
    /// A player is in sight and something blocks the line.
    Blocked,
    /// A player is in sight and the line is open.
    Clear,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct RifleAim {
    pub shooter: Entity,
    /// A clear view pulled the body into the firing pose this frame.
    pub acquired: bool,
    pub look: Look,
    pub muzzle: Vec3,
    pub direction: Vec3,
    pub distance: f32,
}

/// A round left `shooter`. `burst_spent` means the magazine is empty until the reload finishes.
#[derive(Message, Clone, Copy, Debug)]
pub struct RoundFired {
    pub shooter: Entity,
    pub burst_spent: bool,
}

#[derive(Component, Debug)]
pub struct Rifle {
    pub shots_left: u8,
    pub fire_timer: f32,
    pub phase: AttackPhase,
    pub reload: f32,
    pub burst: u8,
    pub damage: u32,
    pub round_speed: f32,
    pub round_radius: f32,
    pub sight: f32,
}

impl Rifle {
    pub fn from_spec(spec: &RifleSpec, opening: f32) -> Self {
        Self {
            shots_left: spec.burst,
            fire_timer: opening,
            phase: AttackPhase::Burst,
            reload: spec.reload.as_secs_f32(),
            burst: spec.burst,
            damage: spec.damage,
            round_speed: spec.round_speed,
            round_radius: spec.round_radius,
            sight: spec.sight,
        }
    }

    pub fn reloading(&self) -> bool {
        self.phase == AttackPhase::Reloading
    }

    /// The burst is full and the rifle is still waiting to open it.
    pub fn holding_fresh_burst(&self) -> bool {
        self.phase == AttackPhase::Burst && self.fire_timer > 0.0 && self.shots_left == self.burst
    }
}

/// Wait between rounds in a burst. A longer reload stretches it a little.
fn burst_gap(reload: f32) -> f32 {
    0.38 + reload * 0.06
}

pub fn fire_rifles(
    time: Res<Time>,
    visuals: Res<Visuals>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut aims: MessageReader<RifleAim>,
    mut fired: MessageWriter<RoundFired>,
    mut rifles: Query<&mut Rifle>,
) {
    let dt = time.delta_secs();
    for aim in aims.read().copied() {
        let Ok(mut rifle) = rifles.get_mut(aim.shooter) else {
            continue;
        };
        if aim.acquired {
            rifle.phase = AttackPhase::Burst;
            rifle.shots_left = rifle.burst;
            rifle.fire_timer = 0.0;
        }
        if rifle.reloading() {
            rifle.fire_timer -= dt;
            if rifle.fire_timer <= 0.0 {
                rifle.phase = AttackPhase::Burst;
                rifle.shots_left = rifle.burst;
                rifle.fire_timer = AFTER_RELOAD;
            }
            continue;
        }
        match aim.look {
            Look::Unseen => {}
            Look::Far => rifle.fire_timer = rifle.fire_timer.min(FAR_WAIT),
            Look::Blocked => rifle.fire_timer = rifle.fire_timer.min(BLOCKED_WAIT),
            Look::Clear => {
                rifle.fire_timer -= dt;
                if rifle.fire_timer > 0.0 {
                    continue;
                }
                if rifle.shots_left == 0 {
                    rifle.phase = AttackPhase::Reloading;
                    rifle.fire_timer = rifle.reload;
                    continue;
                }
                let pitch = 0.94 + (rifle.burst - rifle.shots_left) as f32 * 0.04;
                rifle.shots_left -= 1;
                let burst_spent = rifle.shots_left == 0;
                if burst_spent {
                    rifle.phase = AttackPhase::Reloading;
                    rifle.fire_timer = rifle.reload;
                } else {
                    rifle.fire_timer = burst_gap(rifle.reload);
                }
                commands.spawn((
                    Projectile {
                        direction: aim.direction,
                        speed: rifle.round_speed,
                        radius: rifle.round_radius,
                        previous: aim.muzzle,
                        spent: false,
                        faction: Faction::Enemy,
                        material: Material::Metal,
                        delivery: Delivery::Kinetic,
                        owner: aim.shooter,
                    },
                    Damage(rifle.damage),
                    Lifetime(2.5),
                    Mesh3d(visuals.bullet_mesh.clone()),
                    MeshMaterial3d(visuals.enemy_bullet_material.clone()),
                    Transform::from_translation(aim.muzzle),
                ));
                let volume = (0.26 - aim.distance * 0.004).clamp(0.08, 0.26);
                audio::play_game(&mut commands, sounds.enemy_shot(), volume, pitch);
                fired.write(RoundFired {
                    shooter: aim.shooter,
                    burst_spent,
                });
            }
        }
    }
}
