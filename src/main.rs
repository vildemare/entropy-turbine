mod session;
mod tuning;

mod combat;
mod enemy;
mod player;
mod world;

mod audio;
mod startup;
mod ui;

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
        .init_resource::<session::Session>()
        .init_resource::<enemy::EncounterDirector>()
        .add_systems(Startup, startup::setup)
        .add_systems(Update, (startup::restart, ui::pause::toggle_pause).chain())
        .add_systems(
            Update,
            (
                player::animation::load_animations,
                player::animation::bind_animations,
                player::clear_inactive_intents,
                player::input::read_keyboard_mouse,
                player::move_players,
                world::camera::follow_player,
                player::face_players,
                player::animation::animate_characters,
                player::shoot,
                enemy::place_covers,
                ui::checkpoint::place_checkpoint_line,
                enemy::spawn_groups,
                enemy::move_enemies,
                enemy::separate_enemies,
            )
                .chain()
                .after(ui::pause::toggle_pause),
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
                world::route::follow_route,
                ui::checkpoint::cross_checkpoint,
                ui::update_hud,
            )
                .chain()
                .after(enemy::separate_enemies),
        )
        .add_systems(
            Update,
            (
                ui::pause::handle_pause_buttons,
                ui::checkpoint::handle_shop_buttons,
                ui::checkpoint::handle_checkpoint_button,
                ui::pause::sync_pause_menu,
                ui::checkpoint::sync_checkpoint_menu,
            )
                .chain()
                .after(ui::update_hud),
        )
        .add_systems(
            Update,
            audio::sync_gameplay_audio.after(ui::pause::sync_pause_menu),
        )
        .add_systems(
            Update,
            player::ammo_hud::update.after(ui::checkpoint::sync_checkpoint_menu),
        )
        .add_systems(
            Update,
            ui::update_health_bar.after(ui::checkpoint::sync_checkpoint_menu),
        )
        .add_systems(
            PostUpdate,
            player::animation::aim_upper_body
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        )
        .run();
}
