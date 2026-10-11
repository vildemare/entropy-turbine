//! Pointer capture. Wayland can move the cursor only while it is locked, which
//! is what lets the aim mark follow a sideways camera slide. Menus release the
//! lock and bring the system pointer back.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorMoved, CursorOptions, PrimaryWindow};

use crate::session::{Phase, Session};

const CORE_PX: f32 = 2.0;
const OUTLINE_PX: f32 = 2.0;
const ARM_PX: f32 = 8.0;
const GAP_PX: f32 = 6.0;
const CURSOR_PX: f32 = OUTLINE_PX + ARM_PX + GAP_PX + ARM_PX + OUTLINE_PX;

#[derive(Component)]
pub struct AimCursor;

pub fn pointer_grab(phase: Phase) -> CursorGrabMode {
    if phase == Phase::Playing {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    }
}

pub fn spawn(commands: &mut Commands) {
    let core_at = OUTLINE_PX + ARM_PX + GAP_PX * 0.5 - CORE_PX * 0.5;
    commands
        .spawn((
            AimCursor,
            Node {
                position_type: PositionType::Absolute,
                width: px(CURSOR_PX),
                height: px(CURSOR_PX),
                ..default()
            },
            GlobalZIndex(90),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_children(|cursor| {
            arm(cursor, core_at, OUTLINE_PX, CORE_PX, ARM_PX);
            arm(
                cursor,
                core_at,
                OUTLINE_PX + ARM_PX + GAP_PX,
                CORE_PX,
                ARM_PX,
            );
            arm(cursor, OUTLINE_PX, core_at, ARM_PX, CORE_PX);
            arm(
                cursor,
                OUTLINE_PX + ARM_PX + GAP_PX,
                core_at,
                ARM_PX,
                CORE_PX,
            );
        });
}

fn arm(cursor: &mut ChildSpawnerCommands, left: f32, top: f32, width: f32, height: f32) {
    // The outline is its own node. A border on the 2px core disappears under
    // border-box sizing, which left only a hairline on the dirt.
    cursor.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left - OUTLINE_PX),
            top: px(top - OUTLINE_PX),
            width: px(width + OUTLINE_PX * 2.0),
            height: px(height + OUTLINE_PX * 2.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.02, 0.015, 0.012)),
        ZIndex(0),
        Pickable::IGNORE,
    ));
    cursor.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(width),
            height: px(height),
            ..default()
        },
        BackgroundColor(Color::srgb(0.98, 0.93, 0.74)),
        ZIndex(1),
        Pickable::IGNORE,
    ));
}

/// Fold relative motion into the cursor while play has captured it.
///
/// A locked pointer reports that motion in the same logical pixels as
/// [`Window::cursor_position`], with +x right and +y down the screen. An
/// absolute move this frame has already been written onto the window, so it
/// is left as it is.
pub fn move_captured_pointer(
    session: Res<Session>,
    motion: Res<AccumulatedMouseMotion>,
    mut cursor_moves: MessageReader<CursorMoved>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
) {
    let Ok((entity, mut window)) = windows.single_mut() else {
        return;
    };
    let moved_by_os = cursor_moves.read().any(|event| event.window == entity);
    if session.phase != Phase::Playing {
        return;
    }
    if window.cursor_position().is_none() {
        let center = Vec2::new(window.width() * 0.5, window.height() * 0.5);
        window.set_cursor_position(Some(center));
        return;
    }
    if moved_by_os || motion.delta == Vec2::ZERO {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let max = Vec2::new(
        (window.width() - 1.0).max(0.0),
        (window.height() - 1.0).max(0.0),
    );
    window.set_cursor_position(Some((cursor + motion.delta).clamp(Vec2::ZERO, max)));
}

pub fn sync_pointer_capture(
    session: Res<Session>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut applied: Local<Option<Phase>>,
) {
    if *applied == Some(session.phase) {
        return;
    }
    *applied = Some(session.phase);
    let grab_mode = pointer_grab(session.phase);
    let visible = session.phase != Phase::Playing;
    for mut options in &mut windows {
        if options.grab_mode == grab_mode && options.visible == visible {
            continue;
        }
        options.grab_mode = grab_mode;
        options.visible = visible;
    }
}

pub fn place_aim_cursor(
    session: Res<Session>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cursors: Query<(&mut Node, &mut Visibility), With<AimCursor>>,
) {
    let cursor = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());
    let show = session.phase == Phase::Playing && cursor.is_some();
    for (mut node, mut visibility) in &mut cursors {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(cursor) = cursor {
            node.left = px(cursor.x - CURSOR_PX * 0.5);
            node.top = px(cursor.y - CURSOR_PX * 0.5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_app(phase: Phase) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(Session { phase, ..default() })
            .insert_resource(AccumulatedMouseMotion::default())
            .add_message::<CursorMoved>()
            .add_systems(Update, (move_captured_pointer, sync_pointer_capture));
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1280, 720).into(),
                    ..default()
                },
                PrimaryWindow,
                CursorOptions::default(),
            ))
            .id();
        (app, window)
    }

    #[test]
    fn play_locks_the_pointer_and_menus_release_it() {
        let (mut app, window) = window_app(Phase::Playing);
        app.update();
        let options = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(options.grab_mode, CursorGrabMode::Locked);
        assert!(!options.visible);

        for phase in [Phase::Paused, Phase::Checkpoint, Phase::PlayerDead] {
            app.world_mut().resource_mut::<Session>().phase = phase;
            app.update();
            let options = app.world().get::<CursorOptions>(window).unwrap();
            assert_eq!(options.grab_mode, CursorGrabMode::None, "{phase:?}");
            assert!(options.visible, "{phase:?}");
        }

        app.world_mut().resource_mut::<Session>().phase = Phase::Playing;
        app.update();
        let options = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(options.grab_mode, CursorGrabMode::Locked);
        assert!(!options.visible);
    }

    #[test]
    fn captured_motion_moves_the_pointer_down_and_right() {
        let (mut app, window) = window_app(Phase::Playing);
        {
            let mut cursor_window = app.world_mut().get_mut::<Window>(window).unwrap();
            cursor_window.set_cursor_position(Some(Vec2::new(400.0, 200.0)));
        }
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(30.0, 12.0);
        app.update();
        let cursor = app
            .world()
            .get::<Window>(window)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(cursor, Vec2::new(430.0, 212.0));
    }

    #[test]
    fn captured_motion_stops_at_the_window_edge() {
        let (mut app, window) = window_app(Phase::Playing);
        {
            let mut cursor_window = app.world_mut().get_mut::<Window>(window).unwrap();
            cursor_window.set_cursor_position(Some(Vec2::new(1270.0, 5.0)));
        }
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(40.0, -20.0);
        app.update();
        let cursor = app
            .world()
            .get::<Window>(window)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(cursor, Vec2::new(1279.0, 0.0));
    }

    #[test]
    fn an_absolute_move_is_not_applied_twice() {
        let (mut app, window) = window_app(Phase::Playing);
        {
            let mut cursor_window = app.world_mut().get_mut::<Window>(window).unwrap();
            cursor_window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        }
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(25.0, 0.0);
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(140.0, 100.0),
            delta: Some(Vec2::new(40.0, 0.0)),
        });
        app.update();
        let cursor = app
            .world()
            .get::<Window>(window)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(cursor, Vec2::new(100.0, 100.0));
    }

    #[test]
    fn a_released_pointer_keeps_the_system_position() {
        let (mut app, window) = window_app(Phase::Paused);
        {
            let mut cursor_window = app.world_mut().get_mut::<Window>(window).unwrap();
            cursor_window.set_cursor_position(Some(Vec2::new(80.0, 90.0)));
        }
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(15.0, 15.0);
        app.update();
        let cursor = app
            .world()
            .get::<Window>(window)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(cursor, Vec2::new(80.0, 90.0));
    }
}
