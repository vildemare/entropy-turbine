use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::route;

use crate::player::{LocalPlayer, Player};

pub const CAMERA_HEIGHT: f32 = 24.0;
pub const CAMERA_BACK_OFFSET: f32 = 21.0;

#[derive(Component)]
pub struct FollowCamera;

/// Yaw speed of the follow camera, in radians per second.
#[derive(Component, Default)]
pub struct CameraRig {
    pub(crate) yaw_rate: f32,
}

const YAW_ACCEL: f32 = 1.4;
const YAW_SETTLE_ACCEL: f32 = 0.35;
const YAW_SETTLE_ANGLE: f32 = 0.08;
const MAX_YAW_RATE: f32 = 1.2;

pub fn rear_limit(camera: &Camera, transform: &Transform, window: &Window) -> Option<f32> {
    ground_z_at_viewport_fraction(camera, transform, window, 0.88)
}

/// Ground point in the middle of the view. A level camera falls back to the
/// old point one back-offset ahead of the lens.
pub fn framed_ground(transform: &Transform) -> Vec3 {
    let forward = transform.forward().as_vec3();
    if forward.y >= -0.0001 {
        return Vec3::new(
            transform.translation.x,
            0.0,
            transform.translation.z - CAMERA_BACK_OFFSET,
        );
    }
    let distance = -transform.translation.y / forward.y;
    let hit = transform.translation + forward * distance;
    Vec3::new(hit.x, 0.0, hit.z)
}

pub fn framed_ground_z(transform: &Transform) -> f32 {
    framed_ground(transform).z
}

/// Camera pose that looks along the road through `(look_x, ground_z)`.
pub fn pose_for_ground(look_x: f32, ground_z: f32) -> Transform {
    let slope = route::centerline_slope(ground_z);
    pose_with_heading(look_x, ground_z, (-slope).atan2(1.0))
}

fn pose_with_heading(look_x: f32, ground_z: f32, heading: f32) -> Transform {
    let travel = Vec3::new(heading.sin(), 0.0, -heading.cos());
    let look = Vec3::new(look_x, 0.0, ground_z);
    let mut pose =
        Transform::from_translation(look - travel * CAMERA_BACK_OFFSET + Vec3::Y * CAMERA_HEIGHT);
    pose.look_at(look, Vec3::Y);
    pose
}

fn heading_of(transform: &Transform) -> f32 {
    let forward = transform.forward().as_vec3();
    forward.x.atan2(-forward.z)
}

/// Ease `heading` toward `target`. Speed builds from rest, brakes before the
/// target, and the last few degrees use a smaller push.
fn step_yaw(heading: f32, target: f32, rate: f32, dt: f32) -> (f32, f32) {
    let error = wrap_angle(target - heading);
    if error.abs() < 0.0008 && rate.abs() < 0.02 {
        return (target, 0.0);
    }
    let push_limit = if error.abs() < YAW_SETTLE_ANGLE {
        YAW_SETTLE_ACCEL
    } else {
        YAW_ACCEL
    };
    let approaching = error * rate > 0.0;
    let stopping = rate * rate / (2.0 * YAW_ACCEL);
    let accel = if error.abs() < 0.0008 || (approaching && stopping >= error.abs()) {
        -signed(rate) * YAW_ACCEL
    } else {
        signed(error) * push_limit
    };
    let mut rate = (rate + accel * dt).clamp(-MAX_YAW_RATE, MAX_YAW_RATE);
    if approaching && stopping >= error.abs() && rate * (rate - accel * dt) < 0.0 {
        rate = 0.0;
    }
    (heading + rate * dt, rate)
}

fn signed(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn wrap_angle(angle: f32) -> f32 {
    let mut wrapped = angle % std::f32::consts::TAU;
    if wrapped > std::f32::consts::PI {
        wrapped -= std::f32::consts::TAU;
    } else if wrapped < -std::f32::consts::PI {
        wrapped += std::f32::consts::TAU;
    }
    wrapped
}

/// Ground Z at the top edge of the view. Smaller than the camera center, because
/// forward is −Z. Without a window, this is the same edge the 45° camera reaches.
pub fn visible_front_z(camera: &Camera, transform: &Transform, window: Option<&Window>) -> f32 {
    let center = framed_ground_z(transform);
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
    mut cameras: Query<
        (&Camera, &mut Transform, &mut CameraRig),
        (With<FollowCamera>, Without<Player>),
    >,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    for (camera, mut transform, mut rig) in &mut cameras {
        let before = *transform;
        let framed = framed_ground(&before);
        let mut ground_z = framed.z;
        if player.translation.z < ground_z {
            ground_z += (player.translation.z - ground_z) * (1.0 - (-6.0 * dt).exp());
        }
        let desired_x = route::centerline_x(ground_z);
        let look_x = framed.x + (desired_x - framed.x) * (1.0 - (-8.0 * dt).exp());
        let target = heading_of(&pose_for_ground(look_x, ground_z));
        let (heading, rate) = step_yaw(heading_of(&before), target, rig.yaw_rate, dt);
        rig.yaw_rate = rate;
        *transform = pose_with_heading(look_x, ground_z, heading);
        // A sideways slide or a yaw keeps the pointer on the ground point it
        // already marked. Forward scrolling leaves it alone.
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

/// Where the pointer sits after a sideways slide or a yaw so the same ground
/// point stays under it. A forward-only move returns `None`.
pub fn cursor_after_sideways_move(
    camera: &Camera,
    before: &Transform,
    after: &Transform,
    cursor: Vec2,
) -> Option<Vec2> {
    let before_ground = framed_ground(before);
    let after_ground = framed_ground(after);
    let yawed = before.forward().cross(*after.forward()).y.abs() > 0.0001;
    if (after_ground.x - before_ground.x).abs() < 0.0001 && !yawed {
        return None;
    }
    let world = ground_under_cursor(camera, before, cursor)?;
    let back = after.translation - Vec3::new(after_ground.x, 0.0, after_ground.z);
    let held = Transform::from_translation(Vec3::new(after_ground.x, 0.0, before_ground.z) + back)
        .with_rotation(after.rotation);
    camera
        .world_to_viewport(&GlobalTransform::from(held), world)
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

    #[test]
    fn a_straight_stretch_keeps_the_camera_behind_its_ground_point() {
        let pose = pose_for_ground(2.0, 4.0);
        let ground = framed_ground(&pose);
        assert!((ground.x - 2.0).abs() < 0.001);
        assert!((ground.z - 4.0).abs() < 0.001);
        assert!(pose.forward().as_vec3().x.abs() < 0.001);
    }

    #[test]
    fn a_bend_yaws_the_camera_along_the_road() {
        let mut ground_z = -1.0;
        let mut slope = 0.0;
        for step in 1..500 {
            ground_z = -step as f32;
            slope = route::centerline_slope(ground_z);
            if slope.abs() > 0.2 {
                break;
            }
        }
        assert!(slope.abs() > 0.2);
        let look_x = route::centerline_x(ground_z);
        let pose = pose_for_ground(look_x, ground_z);
        let ground = framed_ground(&pose);
        assert!((ground.x - look_x).abs() < 0.001 && (ground.z - ground_z).abs() < 0.001);
        let forward = pose.forward().as_vec3();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize();
        let travel = Vec3::new(-slope, 0.0, -1.0).normalize();
        assert!(
            (flat - travel).length() < 0.001,
            "forward {flat} travel {travel}"
        );
    }

    #[test]
    fn a_turn_picks_up_speed_from_rest() {
        let first = step_yaw(0.0, 0.5, 0.0, 0.05);
        let mut heading = 0.0;
        let mut rate = 0.0;
        for _ in 0..8 {
            (heading, rate) = step_yaw(heading, 0.5, rate, 0.05);
        }
        let before = heading;
        let (after, later_rate) = step_yaw(heading, 0.5, rate, 0.05);
        assert!(first.0.abs() < (after - before).abs());
        assert!(later_rate > first.1);
    }

    #[test]
    fn a_fast_heading_brakes_as_it_arrives() {
        let (_, rate) = step_yaw(0.0, 0.05, 0.8, 0.05);
        assert!(rate < 0.8);
    }

    #[test]
    fn a_reverse_turn_does_not_flip_the_rate_in_one_step() {
        let (_, rate) = step_yaw(0.3, -0.4, 0.5, 0.016);
        assert!(rate > 0.0);
    }
}
