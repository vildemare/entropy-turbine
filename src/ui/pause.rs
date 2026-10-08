//! Escape menu. Pausing freezes gameplay time and suppresses the next shot.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use bevy::time::Virtual;

use crate::audio::{self, SoundBank};
use crate::session::{Phase, Session};
use crate::ui::UiFonts;

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component, Clone, Copy)]
pub enum PauseAction {
    Continue,
    Exit,
}

pub fn spawn(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.014, 0.012, 0.78)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(430),
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(15),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.09, 0.075)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("ENTROPY TURBINE"),
                        TextFont {
                            font: FontSource::Handle(fonts.heading.clone()),
                            font_size: FontSize::Px(34.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.81, 0.65, 0.39)),
                    ));
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.71, 0.71, 0.65)),
                    ));
                    pause_button(panel, fonts, PauseAction::Continue, "CONTINUE");
                    pause_button(panel, fonts, PauseAction::Exit, "EXIT GAME");
                    panel.spawn((
                        Text::new("ESC  TO CONTINUE"),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.55, 0.53, 0.47)),
                    ));
                });
        });
}

fn pause_button(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    action: PauseAction,
    label: &str,
) {
    panel
        .spawn((
            Button,
            action,
            Node {
                width: percent(100),
                height: px(50),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.20, 0.14)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(fonts.body.clone()),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.84, 0.64)),
            ));
        });
}

pub fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    audio::play_ui(&mut commands, &sounds.menu_click);
    if session.phase == Phase::Paused {
        session.phase = session.resume_phase;
        session.suppress_fire_until_release = true;
        if session.phase == Phase::Checkpoint {
            time.pause();
        } else {
            time.unpause();
        }
    } else {
        session.resume_phase = session.phase;
        session.phase = Phase::Paused;
        time.pause();
    }
}

pub fn handle_pause_buttons(
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    mut exit: MessageWriter<AppExit>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut buttons: Query<
        (&Interaction, &PauseAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, action, mut background) in &mut buttons {
        *background = BackgroundColor(match interaction {
            Interaction::None => Color::srgb(0.24, 0.20, 0.14),
            Interaction::Hovered => Color::srgb(0.34, 0.27, 0.17),
            Interaction::Pressed => Color::srgb(0.48, 0.34, 0.17),
        });
        if session.phase != Phase::Paused || *interaction != Interaction::Pressed {
            continue;
        }
        audio::play_ui(&mut commands, &sounds.menu_click);
        match action {
            PauseAction::Continue => {
                session.phase = session.resume_phase;
                session.suppress_fire_until_release = true;
                if session.phase == Phase::Checkpoint {
                    time.pause();
                } else {
                    time.unpause();
                }
            }
            PauseAction::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub fn sync_pause_menu(
    session: Res<Session>,
    mut overlay: Query<&mut Visibility, With<PauseOverlay>>,
) {
    if !session.is_changed() {
        return;
    }
    for mut visibility in &mut overlay {
        *visibility = if session.phase == Phase::Paused {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_pauses_and_resumes_game_time() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(Time::<Virtual>::default())
            .insert_resource(SoundBank::default())
            .add_systems(Update, (toggle_pause, sync_pause_menu).chain());
        let overlay = app
            .world_mut()
            .spawn((PauseOverlay, Visibility::Hidden))
            .id();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Paused);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            app.world().get::<Visibility>(overlay),
            Some(&Visibility::Visible)
        );

        *app.world_mut().resource_mut::<ButtonInput<KeyCode>>() = ButtonInput::default();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
        assert!(
            app.world()
                .resource::<Session>()
                .suppress_fire_until_release
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(
            app.world().get::<Visibility>(overlay),
            Some(&Visibility::Hidden)
        );
    }
    #[test]
    fn pause_buttons_continue_and_request_exit() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Paused,
            resume_phase: Phase::Playing,
            ..default()
        })
        .insert_resource(Time::<Virtual>::default())
        .insert_resource(SoundBank::default())
        .add_message::<AppExit>()
        .add_systems(Update, handle_pause_buttons);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().spawn((
            Button,
            PauseAction::Continue,
            Interaction::Pressed,
            BackgroundColor(Color::BLACK),
        ));
        app.update();
        assert_eq!(app.world().resource::<Session>().phase, Phase::Playing);
        assert!(
            app.world()
                .resource::<Session>()
                .suppress_fire_until_release
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());

        app.world_mut().resource_mut::<Session>().phase = Phase::Paused;
        app.world_mut().spawn((
            Button,
            PauseAction::Exit,
            Interaction::Pressed,
            BackgroundColor(Color::BLACK),
        ));
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<AppExit>>()
                .drain()
                .next(),
            Some(AppExit::Success)
        );
    }
}
