mod ammo_hud;
mod camera;
mod character;
mod combat;
mod enemy;
mod game;
mod player;
mod sound;
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
        .init_resource::<enemy::EncounterDirector>()
        .add_systems(Startup, game::setup)
        .add_systems(Update, (game::restart, game::toggle_pause).chain())
        .add_systems(
            Update,
            (
                character::load_animations,
                character::bind_animations,
                player::read_input,
                player::move_players,
                camera::follow_player,
                player::face_players,
                character::animate_characters,
                player::shoot,
                enemy::place_covers,
                game::place_checkpoint_line,
                enemy::spawn_groups,
                enemy::move_enemies,
                enemy::separate_enemies,
            )
                .chain()
                .after(game::toggle_pause),
        )
        .add_systems(
            Update,
            (
                enemy::cleanup_behind_camera,
                enemy::shoot_enemies,
                enemy::spawn_reinforcements,
                combat::advance_hit_cooldowns,
                combat::move_projectiles,
                combat::resolve_hits,
                combat::collect_orbs,
                combat::expire_projectiles,
                street::follow_street,
                game::cross_checkpoint,
                game::update_hud,
            )
                .chain()
                .after(enemy::separate_enemies),
        )
        .add_systems(
            Update,
            (
                game::handle_pause_buttons,
                game::handle_shop_buttons,
                game::handle_checkpoint_button,
                game::sync_pause_menu,
                game::sync_checkpoint_menu,
            )
                .chain()
                .after(game::update_hud),
        )
        .add_systems(
            Update,
            sound::sync_gameplay_audio.after(game::sync_pause_menu),
        )
        .add_systems(Update, ammo_hud::update.after(game::sync_checkpoint_menu))
        .add_systems(
            Update,
            game::update_health_bar.after(game::sync_checkpoint_menu),
        )
        .add_systems(
            PostUpdate,
            character::aim_upper_body
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        )
        .run();
}
