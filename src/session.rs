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
    /// The window is up and the run has not started. Continue enters play.
    Title,
    #[default]
    Playing,
    Paused,
    Checkpoint,
    PlayerDead,
}
