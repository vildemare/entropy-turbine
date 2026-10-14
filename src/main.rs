mod session;

mod ability;
mod character;
mod combat;
mod enemy;
mod player;
mod shield;
mod world;

mod audio;
mod startup;
mod ui;

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy::{app::AnimationSystems, transform::TransformSystems};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Entropy Turbine".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            primary_cursor_options: Some(CursorOptions {
                grab_mode: CursorGrabMode::None,
                visible: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(session::Session {
            phase: session::Phase::Title,
            ..default()
        })
        .init_resource::<enemy::EncounterDirector>()
        .add_message::<ability::Struck>()
        .add_message::<ability::Notice>()
        .add_message::<enemy::RifleAim>()
        .add_message::<enemy::RoundFired>()
        .add_message::<enemy::WalkerHold>()
        .add_message::<enemy::BackupStarted>()
        .add_message::<enemy::JoltCue>()
        .add_message::<enemy::ShotsRecalled>()
        .add_message::<player::GearRefresh>()
        .add_plugins(MaterialPlugin::<shield::ShieldMaterial>::default())
        .add_systems(
            Startup,
            (
                startup::setup,
                shield::setup,
                ui::title::pause_until_continue,
            ),
        )
        .add_systems(
            Update,
            (
                startup::restart,
                ui::pause::toggle_pause,
                ui::cursor::move_captured_pointer,
            )
                .chain(),
        )
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
                enemy::spawn_zap_bots,
                enemy::move_enemies,
                enemy::separate_enemies,
                enemy::peel_backups,
                enemy::drive_zap_bots,
                enemy::charge_jolts,
                enemy::aim_grenadiers,
                enemy::walk_grenadiers,
            )
                .chain()
                .after(ui::cursor::move_captured_pointer),
        )
        .add_systems(
            Update,
            (
                (
                    enemy::cleanup_behind_camera,
                    combat::recall_shots,
                    enemy::shoot_enemies,
                    enemy::fire_rifles,
                    enemy::apply_volleys,
                    enemy::spawn_reinforcements,
                    ability::recover_shields,
                    shield::sync_shimmer,
                    combat::advance_hit_cooldowns,
                    combat::move_projectiles,
                    enemy::fly_grenades,
                    combat::resolve_hits,
                    combat::deliver_hits,
                    enemy::credit_hits,
                    combat::present_notices,
                )
                    .chain(),
                (
                    combat::bloom_bursts,
                    combat::collect_orbs,
                    combat::expire_projectiles,
                    world::route::follow_route,
                    ui::checkpoint::cross_checkpoint,
                    ui::update_hud,
                )
                    .chain(),
            )
                .chain()
                .after(enemy::separate_enemies),
        )
        .add_systems(
            Update,
            (
                ui::title::handle_title_button,
                ui::pause::handle_pause_buttons,
                ui::checkpoint::handle_shop_buttons,
                ui::checkpoint::handle_checkpoint_button,
                player::refresh_gear,
                ui::title::sync_title_menu,
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
            (
                ui::cursor::sync_pointer_capture,
                ui::cursor::place_aim_cursor,
            )
                .after(ui::pause::sync_pause_menu),
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
