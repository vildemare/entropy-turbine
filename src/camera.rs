use bevy::prelude::*;

use crate::{
    player::{LocalPlayer, Player},
    street,
};

pub const CAMERA_HEIGHT: f32 = 24.0;
pub const CAMERA_BACK_OFFSET: f32 = 21.0;

#[derive(Component)]
pub struct FollowCamera;

pub fn rear_limit(camera: &Camera, transform: &Transform, window: &Window) -> Option<f32> {
    ground_z_at_viewport_fraction(camera, transform, window, 0.88)
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
    players: Query<&Transform, (With<Player>, With<LocalPlayer>)>,
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<Player>)>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    for mut camera in &mut cameras {
        let centerline = camera.translation.z - CAMERA_BACK_OFFSET;
        if player.translation.z < centerline {
            let target_z = player.translation.z + CAMERA_BACK_OFFSET;
            camera.translation.z +=
                (target_z - camera.translation.z) * (1.0 - (-6.0 * time.delta_secs()).exp());
        }
        let target_x = street::centerline_x(camera.translation.z - CAMERA_BACK_OFFSET);
        camera.translation.x +=
            (target_x - camera.translation.x) * (1.0 - (-8.0 * time.delta_secs()).exp());
    }
}
