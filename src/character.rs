//! Blueprints for the gunslinger and the machines.
//!
//! A definition is the body, the movement, the weapons, and the abilities.
//! Spawn copies those onto an entity. Live timers, position, and remaining
//! health stay on the entity. Speeds and sizes are the floats Bevy moves and
//! draws with. Waits are [`Duration`]s. Health, damage, armor, shields,
//! rewards, and counts stay integers.
//!
//! Cover gait, retreat, and the walker's path are still behavior. They read
//! the blueprint and are not part of it yet.

use std::sync::OnceLock;
use std::time::Duration;

use crate::ability::{Abilities, Ability};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterId {
    Gunslinger,
    Scout,
    Trooper,
    Heavy,
    ZapBot,
    Grenadier,
}

/// Hull, or the gunslinger's collision and health growth.
#[derive(Clone, Copy, Debug)]
pub enum BodySpec {
    Machine {
        health: u32,
        reward: u32,
        radius: f32,
        height: f32,
        visual_scale: f32,
    },
    Playable {
        health: u32,
        health_per_upgrade: u32,
        max_health: u32,
        edge_radius: f32,
        cover_radius: f32,
        hit_radius: f32,
    },
}

/// How the body moves. Path rules stay with the behavior that already owns them.
#[derive(Clone, Copy, Debug)]
pub enum MovementSpec {
    /// Facing comes from the aim. `shoot_move_factor` applies while firing.
    Aimed {
        max_speed: f32,
        shoot_move_factor: f32,
    },
    /// A gunner walking its route.
    Advance { max_speed: f32 },
    /// A tracked machine.
    Tracks { max_speed: f32 },
    /// The grenade walker.
    Walker { max_speed: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct RevolverSpec {
    pub fire_interval: Duration,
    pub starting_capacity: u8,
    pub max_capacity: u8,
    pub starting_reload: Duration,
    pub reload_upgrade: Duration,
    pub min_reload: Duration,
    pub quickload_chamber: Duration,
    pub projectile_speed: f32,
    pub projectile_lifetime: Duration,
    pub projectile_damage: u32,
    pub projectile_radius: f32,
    pub projectile_scale: f32,
    pub pair_spread: f32,
    pub pair_muzzle_offset: f32,
    pub muzzle_forward: f32,
    pub muzzle_side: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RifleSpec {
    pub damage: u32,
    pub reload: Duration,
    pub round_speed: f32,
    pub round_radius: f32,
    pub sight: f32,
    pub burst: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct ZapperSpec {
    pub damage: u32,
    pub charge: Duration,
    pub rest: Duration,
    pub bolt_speed: f32,
    pub reach: f32,
    pub bolt_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LauncherSpec {
    pub engage: f32,
    pub aim: Duration,
    pub count: u8,
    pub reload_each: Duration,
    pub blast_radius: f32,
    pub blast_damage: u32,
    pub flight: Duration,
    pub spread_min: f32,
    pub spread_max: f32,
    pub separation: f32,
    pub arc_peak: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum WeaponSpec {
    Revolver(RevolverSpec),
    Rifle(RifleSpec),
    Zapper(ZapperSpec),
    Launcher(LauncherSpec),
}

/// One step in the order a hit is handled. The values live here.
/// The running pool and the cooldown live on the entity.
#[derive(Clone, Copy, Debug)]
pub enum AbilitySpec {
    StopAlliedShots,
    HitGrace,
    ArmGrace,
    Plate(u32),
    Shield { amount: u32, down_for: Duration },
    HitPoints,
    PlayerLife,
}

impl AbilitySpec {
    pub fn runtime(self) -> Ability {
        match self {
            Self::StopAlliedShots => Ability::StopAlliedShots,
            Self::HitGrace => Ability::HitGrace,
            Self::ArmGrace => Ability::ArmGrace,
            Self::Plate(amount) => Ability::ReduceDamage(amount),
            Self::Shield { .. } => Ability::EnergyShield,
            Self::HitPoints => Ability::HitPoints,
            Self::PlayerLife => Ability::PlayerLife,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CharacterDefinition {
    pub id: CharacterId,
    pub body: BodySpec,
    pub movement: MovementSpec,
    pub weapons: Vec<WeaponSpec>,
    pub abilities: Vec<AbilitySpec>,
}

impl CharacterDefinition {
    pub fn health(&self) -> u32 {
        match self.body {
            BodySpec::Machine { health, .. } | BodySpec::Playable { health, .. } => health,
        }
    }

    pub fn max_health(&self) -> u32 {
        match self.body {
            BodySpec::Machine { health, .. } => health,
            BodySpec::Playable { max_health, .. } => max_health,
        }
    }

    pub fn health_per_upgrade(&self) -> u32 {
        match self.body {
            BodySpec::Machine { .. } => 0,
            BodySpec::Playable {
                health_per_upgrade, ..
            } => health_per_upgrade,
        }
    }

    pub fn reward(&self) -> u32 {
        match self.body {
            BodySpec::Machine { reward, .. } => reward,
            BodySpec::Playable { .. } => 0,
        }
    }

    pub fn radius(&self) -> f32 {
        match self.body {
            BodySpec::Machine { radius, .. } => radius,
            BodySpec::Playable { edge_radius, .. } => edge_radius,
        }
    }

    pub fn height(&self) -> f32 {
        match self.body {
            BodySpec::Machine { height, .. } => height,
            BodySpec::Playable { .. } => panic!("{:?} has no hull height", self.id),
        }
    }

    pub fn visual_scale(&self) -> f32 {
        match self.body {
            BodySpec::Machine { visual_scale, .. } => visual_scale,
            BodySpec::Playable { .. } => 1.0,
        }
    }

    pub fn edge_radius(&self) -> f32 {
        match self.body {
            BodySpec::Playable { edge_radius, .. } => edge_radius,
            BodySpec::Machine { radius, .. } => radius,
        }
    }

    pub fn cover_radius(&self) -> f32 {
        match self.body {
            BodySpec::Playable { cover_radius, .. } => cover_radius,
            BodySpec::Machine { radius, .. } => radius,
        }
    }

    pub fn hit_radius(&self) -> f32 {
        match self.body {
            BodySpec::Playable { hit_radius, .. } => hit_radius,
            BodySpec::Machine { radius, .. } => radius,
        }
    }

    pub fn speed(&self) -> f32 {
        match self.movement {
            MovementSpec::Aimed { max_speed, .. }
            | MovementSpec::Advance { max_speed }
            | MovementSpec::Tracks { max_speed }
            | MovementSpec::Walker { max_speed } => max_speed,
        }
    }

    pub fn shoot_move_factor(&self) -> f32 {
        match self.movement {
            MovementSpec::Aimed {
                shoot_move_factor, ..
            } => shoot_move_factor,
            _ => 1.0,
        }
    }

    pub fn revolver(&self) -> &RevolverSpec {
        self.weapon(|weapon| match weapon {
            WeaponSpec::Revolver(spec) => Some(spec),
            _ => None,
        })
    }

    pub fn rifle(&self) -> &RifleSpec {
        self.weapon(|weapon| match weapon {
            WeaponSpec::Rifle(spec) => Some(spec),
            _ => None,
        })
    }

    pub fn zapper(&self) -> &ZapperSpec {
        self.weapon(|weapon| match weapon {
            WeaponSpec::Zapper(spec) => Some(spec),
            _ => None,
        })
    }

    pub fn launcher(&self) -> &LauncherSpec {
        self.weapon(|weapon| match weapon {
            WeaponSpec::Launcher(spec) => Some(spec),
            _ => None,
        })
    }

    pub fn plate(&self) -> Option<u32> {
        self.abilities.iter().find_map(|ability| match ability {
            AbilitySpec::Plate(amount) => Some(*amount),
            _ => None,
        })
    }

    pub fn shield(&self) -> Option<(u32, Duration)> {
        self.abilities.iter().find_map(|ability| match ability {
            AbilitySpec::Shield { amount, down_for } => Some((*amount, *down_for)),
            _ => None,
        })
    }

    pub fn runtime_abilities(&self) -> Abilities {
        Abilities(
            self.abilities
                .iter()
                .copied()
                .map(AbilitySpec::runtime)
                .collect(),
        )
    }

    fn weapon<'a, T: 'a>(&'a self, pick: impl Fn(&'a WeaponSpec) -> Option<T>) -> T {
        self.weapons
            .iter()
            .find_map(pick)
            .unwrap_or_else(|| panic!("{:?} is missing that weapon", self.id))
    }
}

pub struct CharacterBuilder {
    id: CharacterId,
    body: Option<BodySpec>,
    movement: Option<MovementSpec>,
    weapons: Vec<WeaponSpec>,
    abilities: Vec<AbilitySpec>,
}

impl CharacterBuilder {
    pub fn new(id: CharacterId) -> Self {
        Self {
            id,
            body: None,
            movement: None,
            weapons: Vec::new(),
            abilities: Vec::new(),
        }
    }

    pub fn body(mut self, body: BodySpec) -> Self {
        self.body = Some(body);
        self
    }

    pub fn movement(mut self, movement: MovementSpec) -> Self {
        self.movement = Some(movement);
        self
    }

    pub fn weapon(mut self, weapon: WeaponSpec) -> Self {
        self.weapons.push(weapon);
        self
    }

    pub fn ability(mut self, ability: AbilitySpec) -> Self {
        self.abilities.push(ability);
        self
    }

    pub fn build(self) -> CharacterDefinition {
        CharacterDefinition {
            id: self.id,
            body: self
                .body
                .unwrap_or_else(|| panic!("{:?} has no body", self.id)),
            movement: self
                .movement
                .unwrap_or_else(|| panic!("{:?} has no movement", self.id)),
            weapons: self.weapons,
            abilities: self.abilities,
        }
    }
}

pub fn gunslinger() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| {
        CharacterBuilder::new(CharacterId::Gunslinger)
            .body(BodySpec::Playable {
                health: 1000,
                health_per_upgrade: 200,
                max_health: 5000,
                edge_radius: 0.9,
                cover_radius: 0.65,
                hit_radius: 0.72,
            })
            .movement(MovementSpec::Aimed {
                max_speed: 8.0,
                shoot_move_factor: 0.667,
            })
            .weapon(WeaponSpec::Revolver(RevolverSpec {
                fire_interval: Duration::from_millis(140),
                starting_capacity: 6,
                max_capacity: 20,
                starting_reload: Duration::from_millis(1050),
                reload_upgrade: Duration::from_millis(75),
                min_reload: Duration::from_millis(150),
                quickload_chamber: Duration::from_millis(60),
                projectile_speed: 45.3,
                projectile_lifetime: Duration::from_millis(1_125),
                projectile_damage: 2,
                projectile_radius: 0.113,
                projectile_scale: 0.667,
                pair_spread: 0.009,
                pair_muzzle_offset: 0.025,
                muzzle_forward: 0.95,
                muzzle_side: 0.34,
            }))
            .ability(AbilitySpec::HitGrace)
            .ability(AbilitySpec::PlayerLife)
            .ability(AbilitySpec::ArmGrace)
            .build()
    })
}

pub fn scout() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| machine(CharacterId::Scout, scout_body(), 3.7, 150, 1_150))
}

pub fn trooper() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| machine(CharacterId::Trooper, trooper_body(), 3.0, 200, 1_550))
}

pub fn heavy() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| machine(CharacterId::Heavy, heavy_body(), 2.1, 300, 1_950))
}

pub fn zap_bot() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| {
        CharacterBuilder::new(CharacterId::ZapBot)
            .body(BodySpec::Machine {
                health: 2,
                reward: 1,
                radius: 0.9,
                height: 0.46,
                visual_scale: 1.15,
            })
            .movement(MovementSpec::Tracks { max_speed: 1.5 })
            .weapon(WeaponSpec::Zapper(ZapperSpec {
                damage: 120,
                charge: Duration::from_millis(1_500),
                rest: Duration::from_millis(450),
                bolt_speed: 24.0,
                reach: 8.0,
                bolt_radius: 0.34,
            }))
            .ability(AbilitySpec::StopAlliedShots)
            .ability(AbilitySpec::Plate(1))
            .ability(AbilitySpec::HitPoints)
            .build()
    })
}

pub fn grenadier() -> &'static CharacterDefinition {
    static DEFINITION: OnceLock<CharacterDefinition> = OnceLock::new();
    DEFINITION.get_or_init(|| {
        CharacterBuilder::new(CharacterId::Grenadier)
            .body(BodySpec::Machine {
                health: 4,
                reward: 3,
                radius: 1.35,
                height: 0.95,
                visual_scale: 1.35,
            })
            .movement(MovementSpec::Walker { max_speed: 2.3 })
            .weapon(WeaponSpec::Launcher(LauncherSpec {
                engage: 16.0,
                aim: Duration::from_millis(2_000),
                count: 4,
                reload_each: Duration::from_secs(1),
                blast_radius: 2.0,
                blast_damage: 80,
                flight: Duration::from_millis(900),
                spread_min: 1.25,
                spread_max: 2.35,
                separation: 1.35,
                arc_peak: 4.0,
            }))
            .ability(AbilitySpec::StopAlliedShots)
            .ability(AbilitySpec::Shield {
                amount: 4,
                down_for: Duration::from_secs(4),
            })
            .ability(AbilitySpec::HitPoints)
            .build()
    })
}

fn machine(
    id: CharacterId,
    body: BodySpec,
    speed: f32,
    damage: u32,
    reload_ms: u64,
) -> CharacterDefinition {
    CharacterBuilder::new(id)
        .body(body)
        .movement(MovementSpec::Advance { max_speed: speed })
        .weapon(WeaponSpec::Rifle(RifleSpec {
            damage,
            reload: Duration::from_millis(reload_ms),
            round_speed: 13.0,
            round_radius: 0.17,
            sight: 33.0,
            burst: 3,
        }))
        .ability(AbilitySpec::StopAlliedShots)
        .ability(AbilitySpec::HitPoints)
        .build()
}

fn scout_body() -> BodySpec {
    BodySpec::Machine {
        health: 3,
        reward: 1,
        radius: 0.44,
        height: 0.54,
        visual_scale: 0.80,
    }
}

fn trooper_body() -> BodySpec {
    BodySpec::Machine {
        health: 6,
        reward: 2,
        radius: 0.56,
        height: 0.68,
        visual_scale: 1.0,
    }
}

fn heavy_body() -> BodySpec {
    BodySpec::Machine {
        health: 15,
        reward: 4,
        radius: 0.73,
        height: 0.83,
        visual_scale: 1.22,
    }
}
