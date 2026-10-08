//! Enemies and the cover they fight from.
//!
//! `encounter` decides when a wave exists. `behavior` decides how each enemy moves and shoots.

use bevy::prelude::*;

mod behavior;
mod encounter;

pub use crate::tuning::EnemyKind;
pub use behavior::{
    cleanup_behind_camera, move_enemies, separate_enemies, separate_from_cover, shoot_enemies,
};
pub use encounter::{
    EncounterDirector, WAVES_PER_PHASE, place_covers, spawn_groups, spawn_reinforcements,
};

#[derive(Component)]
pub struct EnemyReward {
    pub kind: EnemyKind,
    pub hit_player: bool,
}

impl EnemyReward {
    pub fn points(&self) -> u32 {
        self.kind.profile().reward_points + u32::from(self.hit_player)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_shot_damage_uses_player_health_scale() {
        assert_eq!(EnemyKind::Scout.profile().shot_damage, 150);
        assert_eq!(EnemyKind::Trooper.profile().shot_damage, 200);
        assert_eq!(EnemyKind::Heavy.profile().shot_damage, 300);
        assert_eq!(EnemyKind::Scout.profile().shot_speed, 13.0);
        assert_eq!(EnemyKind::Trooper.profile().shot_speed, 13.0);
        assert_eq!(EnemyKind::Heavy.profile().shot_speed, 13.0);
    }
}
