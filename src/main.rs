mod camera;
mod character;
mod combat;
mod enemy;
mod game;
mod player;
mod street;

use bevy::prelude::*;
use bevy::{app::AnimationSystems, transform::TransformSystems};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Entropy Turbine".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<game::Session>()
        .init_resource::<enemy::SpawnClock>()
        .add_systems(Startup, game::setup)
        .add_systems(
            Update,
            (
                game::restart,
                character::load_animations,
                character::bind_animations,
                player::read_input,
                player::move_players,
                player::face_players,
                character::animate_characters,
                player::shoot,
                enemy::spawn_enemies,
                enemy::chase_players,
                combat::move_projectiles,
                combat::hit_enemies,
                combat::contact_damage,
                combat::expire_projectiles,
                camera::follow_player,
                street::follow_street,
                game::update_hud,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            character::aim_upper_body
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        )
        .run();
}
