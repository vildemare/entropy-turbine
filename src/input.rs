use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    camera::FollowCamera,
    game::{Phase, Session},
    player::{KeyboardMouseControlled, Player, PlayerIntent},
};

// Device adapters write PlayerIntent before the gameplay systems run. Another
// adapter can write the same component for its own player without changing
// movement, aiming, or weapons.
pub fn read_keyboard_mouse(
    mut session: ResMut<Session>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<FollowCamera>>,
    mut players: Query<&mut PlayerIntent, (With<Player>, With<KeyboardMouseControlled>)>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    if session.suppress_fire_until_release && !mouse.pressed(MouseButton::Left) {
        session.suppress_fire_until_release = false;
    }
    let movement = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8) as f32,
        (keys.pressed(KeyCode::KeyS) as i8 - keys.pressed(KeyCode::KeyW) as i8) as f32,
    )
    .normalize_or_zero();
    let aim = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            let (camera, transform) = camera.single().ok()?;
            let ray = camera.viewport_to_world(transform, cursor).ok()?;
            let direction = *ray.direction;
            if direction.y.abs() < 0.0001 {
                return None;
            }
            let distance = -ray.origin.y / direction.y;
            (distance > 0.0).then_some(ray.origin + direction * distance)
        });

    for mut intent in &mut players {
        intent.movement = movement;
        intent.firing = mouse.pressed(MouseButton::Left) && !session.suppress_fire_until_release;
        if let Some(aim) = aim {
            intent.aim = aim;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_input_only_writes_its_assigned_player() {
        let mut app = App::new();
        app.insert_resource(Session::default())
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(ButtonInput::<MouseButton>::default())
            .add_systems(Update, read_keyboard_mouse);
        let local = app
            .world_mut()
            .spawn((Player, KeyboardMouseControlled, PlayerIntent::default()))
            .id();
        let external = app
            .world_mut()
            .spawn((
                Player,
                PlayerIntent {
                    movement: Vec2::NEG_X,
                    aim: Vec3::new(3.0, 0.0, -4.0),
                    firing: false,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        let local_intent = app.world().get::<PlayerIntent>(local).unwrap();
        assert_eq!(local_intent.movement, Vec2::X);
        assert!(local_intent.firing);
        let external_intent = app.world().get::<PlayerIntent>(external).unwrap();
        assert_eq!(external_intent.movement, Vec2::NEG_X);
        assert!(!external_intent.firing);
    }

    #[test]
    fn pausing_clears_intent_from_every_source() {
        let mut app = App::new();
        app.insert_resource(Session {
            phase: Phase::Paused,
            ..default()
        })
        .add_systems(Update, crate::player::clear_inactive_intents);
        let player = app
            .world_mut()
            .spawn((
                Player,
                PlayerIntent {
                    movement: Vec2::X,
                    aim: Vec3::new(2.0, 0.0, -3.0),
                    firing: true,
                },
            ))
            .id();
        app.update();
        let intent = app.world().get::<PlayerIntent>(player).unwrap();
        assert_eq!(intent.movement, Vec2::ZERO);
        assert!(!intent.firing);
        assert_eq!(intent.aim, Vec3::new(2.0, 0.0, -3.0));
    }
}
