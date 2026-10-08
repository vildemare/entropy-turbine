//! Base tuning for actors and weapons. Runtime state (health, loaded rounds,
//! cooldowns, AI phase) belongs to entities; these profiles define defaults.

#[derive(Clone, Copy, Debug)]
pub struct PlayerBase {
    pub move_speed: f32,
    pub shoot_move_factor: f32,
    pub starting_health: u32,
    pub health_per_upgrade: u32,
    pub max_health: u32,
    pub edge_radius: f32,
    pub cover_radius: f32,
    pub hit_radius: f32,
}

pub const GUNSLINGER_PLAYER: PlayerBase = PlayerBase {
    move_speed: 8.0,
    shoot_move_factor: 2.0 / 3.0,
    starting_health: 1000,
    health_per_upgrade: 200,
    max_health: 3000,
    edge_radius: 0.9,
    cover_radius: 0.65,
    hit_radius: 0.72,
};

#[derive(Clone, Copy, Debug)]
pub struct WeaponProfile {
    pub fire_interval_ms: u64,
    pub starting_capacity: u8,
    pub max_capacity: u8,
    pub starting_reload_wait_ms: u32,
    pub reload_upgrade_ms: u32,
    pub min_reload_wait_ms: u32,
    pub quickload_chamber_ms: u64,
    pub projectile_speed: f32,
    pub projectile_lifetime: f32,
    pub projectile_damage: u32,
    pub projectile_radius: f32,
    pub projectile_visual_scale: f32,
    pub pair_spread: f32,
    pub pair_muzzle_offset: f32,
    pub muzzle_forward: f32,
    pub muzzle_side: f32,
}

pub const GUNSLINGER_WEAPON: WeaponProfile = WeaponProfile {
    fire_interval_ms: 140,
    starting_capacity: 6,
    max_capacity: 12,
    starting_reload_wait_ms: 1050,
    reload_upgrade_ms: 75,
    min_reload_wait_ms: 600,
    quickload_chamber_ms: 60,
    projectile_speed: 34.0 * 4.0 / 3.0,
    projectile_lifetime: 1.125, // about the same 51 metre range as before
    projectile_damage: 2,
    projectile_radius: 0.17 * 2.0 / 3.0,
    projectile_visual_scale: 2.0 / 3.0,
    pair_spread: 0.009,
    pair_muzzle_offset: 0.025,
    muzzle_forward: 0.95,
    muzzle_side: 0.34,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Scout,
    Trooper,
    Heavy,
}

#[derive(Clone, Copy, Debug)]
pub struct EnemyProfile {
    pub health: u32,
    pub radius: f32,
    pub height: f32,
    pub move_speed: f32,
    pub reload_secs: f32,
    pub shot_damage: u32,
    pub shot_speed: f32,
    pub shot_radius: f32,
    pub reward_points: u32,
    pub visual_scale: f32,
}

impl EnemyKind {
    pub const fn profile(self) -> EnemyProfile {
        match self {
            Self::Scout => EnemyProfile {
                health: 3,
                radius: 0.44,
                height: 0.54,
                move_speed: 3.7,
                reload_secs: 1.15,
                shot_damage: 150,
                shot_speed: 16.0,
                shot_radius: 0.17,
                reward_points: 1,
                visual_scale: 0.80,
            },
            Self::Trooper => EnemyProfile {
                health: 6,
                radius: 0.56,
                height: 0.68,
                move_speed: 3.0,
                reload_secs: 1.55,
                shot_damage: 200,
                shot_speed: 16.0,
                shot_radius: 0.17,
                reward_points: 2,
                visual_scale: 1.0,
            },
            Self::Heavy => EnemyProfile {
                health: 15,
                radius: 0.73,
                height: 0.83,
                move_speed: 2.1,
                reload_secs: 1.95,
                shot_damage: 300,
                shot_speed: 16.0,
                shot_radius: 0.17,
                reward_points: 4,
                visual_scale: 1.22,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnemyBehavior {
    pub sight_range: f32,
    pub burst_shots: u8,
    pub cover_entry_speed_factor: f32,
    pub cover_exit_speed_factor: f32,
    pub retreat_speed: f32,
    pub retreat_timeout: f32,
}

pub const ENEMY_BEHAVIOR: EnemyBehavior = EnemyBehavior {
    sight_range: 33.0,
    burst_shots: 3,
    cover_entry_speed_factor: 1.5,
    cover_exit_speed_factor: 1.5,
    retreat_speed: 6.0,
    retreat_timeout: 3.5,
};
