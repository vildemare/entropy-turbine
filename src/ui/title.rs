//! The screen that holds the run until Continue. The clock stays put while
//! the window finishes loading.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use bevy::time::Virtual;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

use crate::audio::{self, SoundBank};
use crate::session::{Phase, Session};
use crate::ui::UiFonts;

#[derive(Component)]
pub struct TitleOverlay;

#[derive(Component)]
pub struct TitleContinue;

pub fn spawn(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            TitleOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.014, 0.012, 0.78)),
            GlobalZIndex(110),
            Visibility::Visible,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(430),
                        padding: UiRect::all(px(28)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(18),
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
                    panel
                        .spawn((
                            Button,
                            Hovered::default(),
                            TitleContinue,
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
                                Text::new("CONTINUE"),
                                TextFont {
                                    font: FontSource::Handle(fonts.body.clone()),
                                    font_size: FontSize::Px(22.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.94, 0.84, 0.64)),
                            ));
                        });
                });
        });
}

/// Startup only. Continue is what releases the clock.
pub fn pause_until_continue(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

pub fn handle_title_button(
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    sounds: Res<SoundBank>,
    mut commands: Commands,
    mut colors: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<TitleContinue>>,
    pressed: Query<(), (Added<Pressed>, With<TitleContinue>)>,
) {
    for (hovered, held, mut background) in &mut colors {
        *background = BackgroundColor(if held {
            Color::srgb(0.48, 0.34, 0.17)
        } else if hovered.get() {
            Color::srgb(0.34, 0.27, 0.17)
        } else {
            Color::srgb(0.24, 0.20, 0.14)
        });
    }
    if session.phase != Phase::Title || pressed.is_empty() {
        return;
    }
    audio::play_ui(&mut commands, &sounds.menu_click);
    session.phase = Phase::Playing;
    session.suppress_fire_until_release = true;
    time.unpause();
}

pub fn sync_title_menu(
    session: Res<Session>,
    mut overlay: Query<&mut Visibility, With<TitleOverlay>>,
) {
    if !session.is_changed() {
        return;
    }
    for mut visibility in &mut overlay {
        *visibility = if session.phase == Phase::Title {
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
    fn continue_starts_the_clock_and_hides_the_title() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Title,
            ..default()
        })
        .insert_resource(Time::<Virtual>::default())
        .insert_resource(SoundBank::default())
        .add_systems(Update, (handle_title_button, sync_title_menu).chain());
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let overlay = app
            .world_mut()
            .spawn((TitleOverlay, Visibility::Visible))
            .id();
        app.world_mut().spawn((
            Button,
            TitleContinue,
            Pressed,
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
        assert_eq!(
            app.world().get::<Visibility>(overlay),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn the_survival_clock_stays_put_on_the_title() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Title,
            ..default()
        })
        .insert_resource(crate::enemy::EncounterDirector::default())
        .insert_resource(Time::<()>::default())
        .add_systems(Update, crate::ui::update_hud);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(3));
        app.update();
        assert_eq!(app.world().resource::<Session>().elapsed, 0.0);
    }
}
