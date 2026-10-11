use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::route;

use crate::player::{LocalPlayer, Player};

pub const CAMERA_HEIGHT: f32 = 24.0;
pub const CAMERA_BACK_OFFSET: f32 = 21.0;

#[derive(Component)]
pub struct FollowCamera;

pub fn rear_limit(camera: &Camera, transform: &Transform, window: &Window) -> Option<f32> {
    ground_z_at_viewport_fraction(camera, transform, window, 0.88)
}

/// Ground Z at the top edge of the view. Smaller than the camera center, because
/// forward is −Z. Without a window, this is the same edge the 45° camera reaches.
pub fn visible_front_z(camera: &Camera, transform: &Transform, window: Option<&Window>) -> f32 {
    let center = transform.translation.z - CAMERA_BACK_OFFSET;
    window
        .and_then(|window| ground_z_at_viewport_fraction(camera, transform, window, 0.0))
        .unwrap_or(center - 28.0)
}

pub fn ground_z_at_viewport_fraction(
    camera: &Camera,
    transform: &Transform,
    window: &Window,
    vertical_fraction: f32,
) -> Option<f32> {
    let viewport_point = Vec2::new(window.width() * 0.5, window.height() * vertical_fraction);
    let ray = camera
        .viewport_to_world(&GlobalTransform::from(*transform), viewport_point)
        .ok()?;
    let direction = *ray.direction;
    if direction.y >= -0.0001 {
        return None;
    }
    let distance = -ray.origin.y / direction.y;
    (distance > 0.0).then_some(ray.origin.z + direction.z * distance)
}

pub fn follow_player(
    time: Res<Time>,
    players: Query<&Transform, (With<Player>, With<LocalPlayer>, Without<FollowCamera>)>,
    mut cameras: Query<(&Camera, &mut Transform), (With<FollowCamera>, Without<Player>)>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    for (camera, mut transform) in &mut cameras {
        let before = *transform;
        let centerline = transform.translation.z - CAMERA_BACK_OFFSET;
        if player.translation.z < centerline {
            let target_z = player.translation.z + CAMERA_BACK_OFFSET;
            transform.translation.z +=
                (target_z - transform.translation.z) * (1.0 - (-6.0 * time.delta_secs()).exp());
        }
        let target_x = route::centerline_x(transform.translation.z - CAMERA_BACK_OFFSET);
        transform.translation.x +=
            (target_x - transform.translation.x) * (1.0 - (-8.0 * time.delta_secs()).exp());
        // The road's sideways slide moves the view. Keep the pointer on the
        // ground point it already marked. Forward scrolling leaves it alone.
        if let Ok(mut window) = windows.single_mut()
            && let Some(cursor) = window.cursor_position()
            && let Some(next) = cursor_after_sideways_move(camera, &before, &transform, cursor)
        {
            let max = Vec2::new(
                (window.width() - 1.0).max(0.0),
                (window.height() - 1.0).max(0.0),
            );
            let clamped = next.clamp(Vec2::ZERO, max);
            if (clamped - cursor).length_squared() >= 0.25 {
                window.set_cursor_position(Some(clamped));
            }
        }
    }
}

/// Where the pointer sits after a sideways camera move so the same ground
/// point stays under it. A forward-only move returns `None`.
pub fn cursor_after_sideways_move(
    camera: &Camera,
    before: &Transform,
    after: &Transform,
    cursor: Vec2,
) -> Option<Vec2> {
    if (after.translation.x - before.translation.x).abs() < 0.0001 {
        return None;
    }
    let world = ground_under_cursor(camera, before, cursor)?;
    let mut lateral = *after;
    lateral.translation.y = before.translation.y;
    lateral.translation.z = before.translation.z;
    camera
        .world_to_viewport(&GlobalTransform::from(lateral), world)
        .ok()
}

fn ground_under_cursor(camera: &Camera, transform: &Transform, cursor: Vec2) -> Option<Vec3> {
    let ray = camera
        .viewport_to_world(&GlobalTransform::from(*transform), cursor)
        .ok()?;
    let direction = *ray.direction;
    if direction.y.abs() < 0.0001 {
        return None;
    }
    let distance = -ray.origin.y / direction.y;
    (distance > 0.0).then_some(ray.origin + direction * distance)
}

#[cfg(test)]
mod tests {
    use bevy::camera::{CameraProjection, PerspectiveProjection, RenderTargetInfo, Viewport};

    use super::*;

    fn game_camera() -> Camera {
        let size = Vec2::new(1600.0, 900.0);
        let mut projection = PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_4,
            ..Default::default()
        };
        projection.update(size.x, size.y);
        let mut camera = Camera {
            viewport: Some(Viewport {
                physical_size: size.as_uvec2(),
                ..Default::default()
            }),
            ..Default::default()
        };
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: size.as_uvec2(),
            scale_factor: 1.0,
        });
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera
    }

    fn game_view(x: f32, z: f32) -> Transform {
        Transform::from_xyz(x, CAMERA_HEIGHT, z)
            .looking_at(Vec3::new(x, 0.0, z - CAMERA_BACK_OFFSET), Vec3::Y)
    }

    #[test]
    fn a_sideways_camera_move_keeps_the_pointer_on_the_same_ground_point() {
        let camera = game_camera();
        let before = game_view(0.0, CAMERA_BACK_OFFSET);
        let cursor = Vec2::new(800.0, 450.0);
        let world = ground_under_cursor(&camera, &before, cursor).unwrap();
        let after = game_view(6.0, CAMERA_BACK_OFFSET);
        let next = cursor_after_sideways_move(&camera, &before, &after, cursor).unwrap();
        assert!(
            next.x < cursor.x,
            "camera moving +X should slide the pointer left, {next}"
        );
        let kept = ground_under_cursor(&camera, &after, next).unwrap();
        assert!(
            (kept.x - world.x).abs() < 0.05 && (kept.z - world.z).abs() < 0.05,
            "ground moved from {world} to {kept}"
        );
    }

    #[test]
    fn forward_scrolling_does_not_drag_the_pointer() {
        let camera = game_camera();
        let before = game_view(2.0, CAMERA_BACK_OFFSET);
        let after = game_view(2.0, CAMERA_BACK_OFFSET - 4.0);
        let cursor = Vec2::new(640.0, 300.0);
        assert!(cursor_after_sideways_move(&camera, &before, &after, cursor).is_none());
    }
}
