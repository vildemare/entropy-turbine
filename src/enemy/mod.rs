//! Enemies and the cover they fight from.
//!
//! `encounter` decides when a wave exists. `behavior` decides how each enemy moves and shoots.

use bevy::prelude::*;

mod behavior;
mod encounter;
mod grenadier;
mod zap_bot;

pub use behavior::{
    Drift, cleanup_behind_camera, move_enemies, separate_enemies, separate_from_cover,
    shoot_enemies,
};
pub use encounter::{
    EncounterDirector, WAVES_PER_PHASE, place_covers, spawn_groups, spawn_reinforcements,
};
pub use grenadier::{Grenadier, VolleyPiece, fly_grenades, walk_grenadiers};
pub use zap_bot::{ZapBot, drive_zap_bots, spawn_zap_bots};

/// Which machine this is. The bounty is the one number the kind itself owns;
/// health, pace, and weapons live with those capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Scout,
    Trooper,
    Heavy,
    ZapBot,
    Grenadier,
}

/// Health, size, bounty, and mesh scale.
///
/// `radius` and `height` are units. 1000 units is 1 metre. Scale is a factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub health: u32,
    pub reward: u32,
    pub radius: u32,
    pub height: u32,
    pub visual_scale: f32,
}

/// How fast a body walks, in units. 1000 units is 1 metre per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pace {
    pub speed: u32,
}

#[derive(Component)]
pub struct EnemyReward {
    pub kind: EnemyKind,
    pub hit_player: bool,
}

impl EnemyReward {
    pub fn points(&self) -> u32 {
        let bounty = match self.kind {
            EnemyKind::Scout => behavior::SCOUT.hull.reward,
            EnemyKind::Trooper => behavior::TROOPER.hull.reward,
            EnemyKind::Heavy => behavior::HEAVY.hull.reward,
            EnemyKind::ZapBot => zap_bot::ZAP_HULL.reward,
            EnemyKind::Grenadier => grenadier::WALKER_HULL.reward,
        };
        bounty + u32::from(self.hit_player)
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
    pub drift: Drift,
}

#[derive(Component)]
pub struct Cover {
    pub half_width: f32,
    pub half_depth: f32,
    pub height: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_shot_damage_uses_player_health_scale() {
        assert_eq!(behavior::SCOUT.rifle.damage, 150);
        assert_eq!(behavior::TROOPER.rifle.damage, 200);
        assert_eq!(behavior::HEAVY.rifle.damage, 300);
        assert_eq!(behavior::RIFLE_ROUND.speed, 13_000);
        assert_eq!(
            behavior::SCOUT.rifle.round.speed,
            behavior::RIFLE_ROUND.speed
        );
        assert_eq!(
            behavior::TROOPER.rifle.round.speed,
            behavior::RIFLE_ROUND.speed
        );
        assert_eq!(
            behavior::HEAVY.rifle.round.speed,
            behavior::RIFLE_ROUND.speed
        );
        assert_eq!(zap_bot::ZAP_HULL.health, 2);
        let through = crate::player::GUNSLINGER_WEAPON.projectile_damage - zap_bot::ZAP_ARMOR;
        assert!(through > 0, "the plate stays under the current shot");
        assert!(
            through * 2 >= zap_bot::ZAP_HULL.health,
            "one paired hit still drops a cow"
        );
        assert_eq!(grenadier::WALKER_HULL.health, 4);
        assert_eq!(grenadier::WALKER_SHIELD, 4);
        assert_eq!(grenadier::WALKER_SHIELD_DOWN, 4_000);
    }
}
