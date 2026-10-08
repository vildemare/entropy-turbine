//! Shared run state. Gameplay reads `Phase` instead of asking the menus.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Session {
    pub phase: Phase,
    pub resume_phase: Phase,
    pub suppress_fire_until_release: bool,
    pub kills: u32,
    pub points: u32,
    pub elapsed: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Playing,
    Paused,
    Checkpoint,
    PlayerDead,
}
