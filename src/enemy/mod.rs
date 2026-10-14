//! Enemies and the cover they fight from.
//!
//! `encounter` decides when a wave exists. `gunner` is the scout, the trooper,
//! and the heavy. `behavior` steps every machine and keeps bodies apart.
//! `rifle` owns the burst and the round. The grenadier's launcher and the
//! cow's jolt own their own shots.

use bevy::prelude::*;

mod behavior;
mod encounter;
mod grenadier;
mod gunner;
mod rifle;
mod zap_bot;

pub use behavior::{
    BackupStarted, Drift, WalkerHold, separate_enemies, separate_from_cover, walk_grenadiers,
};
pub use encounter::{
    EncounterDirector, WAVES_PER_PHASE, place_covers, spawn_groups, spawn_reinforcements,
};
pub use grenadier::{Grenadier, VolleyPiece, aim_grenadiers, fly_grenades};
pub use gunner::{
    ShotsRecalled, apply_volleys, cleanup_behind_camera, move_enemies, peel_backups, shoot_enemies,
};
pub use rifle::{Rifle, RifleAim, RoundFired, fire_rifles};
pub use zap_bot::{JoltCue, ZapBot, charge_jolts, drive_zap_bots, spawn_zap_bots};

/// A player was hurt. The machine that did it marks its own bounty.
pub fn credit_hits(
    mut notices: MessageReader<crate::ability::Notice>,
    mut rewards: Query<&mut EnemyReward>,
) {
    for notice in notices.read().copied() {
        let crate::ability::Notice::Hurt { player: true, by } = notice else {
            continue;
        };
        let Ok(mut reward) = rewards.get_mut(by) else {
            continue;
        };
        reward.hit_player = true;
    }
}

/// Which machine this is. Health, pace, and weapons live on its
/// [`crate::character::CharacterDefinition`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Scout,
    Trooper,
    Heavy,
    ZapBot,
    Grenadier,
}

impl EnemyKind {
    pub fn definition(self) -> &'static crate::character::CharacterDefinition {
        match self {
            Self::Scout => crate::character::scout(),
            Self::Trooper => crate::character::trooper(),
            Self::Heavy => crate::character::heavy(),
            Self::ZapBot => crate::character::zap_bot(),
            Self::Grenadier => crate::character::grenadier(),
        }
    }
}

#[derive(Component)]
pub struct EnemyReward {
    pub kind: EnemyKind,
    pub hit_player: bool,
}

impl EnemyReward {
    pub fn points(&self) -> u32 {
        let bounty = self.kind.definition().reward();
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
    pub evade_direction: f32,
    pub sight_blocked: bool,
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
    #[test]
    fn enemy_shot_damage_uses_player_health_scale() {
        use std::time::Duration;

        let scout = crate::character::scout();
        let trooper = crate::character::trooper();
        let heavy = crate::character::heavy();
        assert_eq!(scout.rifle().damage, 150);
        assert_eq!(trooper.rifle().damage, 200);
        assert_eq!(heavy.rifle().damage, 300);
        assert_eq!(scout.rifle().round_speed, 13.0);
        assert_eq!(trooper.rifle().round_speed, scout.rifle().round_speed);
        assert_eq!(heavy.rifle().round_speed, scout.rifle().round_speed);
        let zap = crate::character::zap_bot();
        assert_eq!(zap.health(), 2);
        let through = crate::character::gunslinger().revolver().projectile_damage
            - zap.plate().expect("the cow wears a plate");
        assert!(through > 0, "the plate stays under the current shot");
        assert!(
            through * 2 >= zap.health(),
            "one paired hit still drops a cow"
        );
        let walker = crate::character::grenadier();
        assert_eq!(walker.health(), 4);
        let (shield, down_for) = walker.shield().expect("the walker wears a shield");
        assert_eq!(shield, 4);
        assert_eq!(down_for, Duration::from_secs(4));
    }
}
