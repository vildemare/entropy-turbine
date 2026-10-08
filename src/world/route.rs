use bevy::prelude::*;
use bevy::{asset::RenderAssetUsages, render::render_resource::PrimitiveTopology};

use crate::player::{LocalPlayer, Player};

pub const ROAD_HALF_WIDTH: f32 = 13.0;
pub(crate) const BOULDER_SIZE: Vec3 = Vec3::new(2.8, 1.5, 3.5);
const VISIBLE_LENGTH: f32 = 300.0;
const BOULDER_SPACING: f32 = 3.0;
const BOULDER_HALF_COUNT: i32 = 48;
const BOULDER_COUNT: i32 = BOULDER_HALF_COUNT * 2 + 1;
const BEND_LENGTH: f32 = 90.0;
const MAX_OFFSET: f32 = 4.0;

#[derive(Component)]
pub(crate) struct TerrainVisual;

#[derive(Component)]
pub(crate) struct Boulder(pub i32);

#[derive(Resource)]
pub(crate) struct RoadMesh {
    handle: Handle<Mesh>,
    center_segment: i32,
}

// A cubic Bezier with coincident end handles gives horizontal tangents at
// every control point. Adjacent offsets differ by at most eight metres, so
// the maximum slope is 1.5 * 8 / 90, below the requested 15%.
pub fn centerline_x(z: f32) -> f32 {
    if z >= 0.0 {
        return 0.0;
    }
    let progress = -z / BEND_LENGTH;
    let segment = progress.floor() as u32;
    let t = progress.fract();
    let eased = t * t * (3.0 - 2.0 * t);
    offset_at(segment) + (offset_at(segment + 1) - offset_at(segment)) * eased
}

pub fn centerline_slope(z: f32) -> f32 {
    if z >= 0.0 {
        return 0.0;
    }
    let progress = -z / BEND_LENGTH;
    let segment = progress.floor() as u32;
    let t = progress.fract();
    -(offset_at(segment + 1) - offset_at(segment)) * 6.0 * t * (1.0 - t) / BEND_LENGTH
}

fn offset_at(segment: u32) -> f32 {
    if segment == 0 {
        return 0.0;
    }
    let mut value = segment.wrapping_mul(0x9e37_79b9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x85eb_ca6b);
    value ^= value >> 13;
    // A few flat sections make the bends occasional rather than constant.
    if value % 5 == 0 {
        0.0
    } else {
        (value % 801) as f32 * 0.01 - MAX_OFFSET
    }
}

/// Keep an actor's center inside the boulder lined playfield.
pub fn clamp_actor_x(x: f32, z: f32, radius: f32) -> f32 {
    let center = centerline_x(z);
    x.clamp(
        center - ROAD_HALF_WIDTH + radius,
        center + ROAD_HALF_WIDTH - radius,
    )
}

fn road_positions(center_z: f32) -> Vec<[f32; 3]> {
    let mut positions = Vec::with_capacity(600);
    for index in 0..100 {
        let z0 = center_z + VISIBLE_LENGTH * 0.5 - index as f32 * 3.0;
        let z1 = z0 - 3.0;
        let a = centerline_x(z0);
        let b = centerline_x(z1);
        let left0 = [a - ROAD_HALF_WIDTH, 0.0, z0 - center_z];
        let right0 = [a + ROAD_HALF_WIDTH, 0.0, z0 - center_z];
        let left1 = [b - ROAD_HALF_WIDTH, 0.0, z1 - center_z];
        let right1 = [b + ROAD_HALF_WIDTH, 0.0, z1 - center_z];
        positions.extend_from_slice(&[left0, right0, right1, left0, right1, left1]);
    }
    positions
}

fn road_mesh(center_z: f32) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, road_positions(center_z));
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 600]);
    mesh
}

pub fn setup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(120.0, VISIBLE_LENGTH))),
        MeshMaterial3d(materials.add(Color::srgb(0.065, 0.055, 0.046))),
        Transform::from_xyz(0.0, -0.04, 0.0),
        TerrainVisual,
    ));
    let road_handle = meshes.add(road_mesh(0.0));
    commands.insert_resource(RoadMesh {
        handle: road_handle.clone(),
        center_segment: 0,
    });
    commands.spawn((
        Mesh3d(road_handle),
        MeshMaterial3d(materials.add(Color::srgb(0.20, 0.15, 0.105))),
        Transform::default(),
        TerrainVisual,
    ));

    let boulder_mesh = meshes.add(Cuboid::new(BOULDER_SIZE.x, BOULDER_SIZE.y, BOULDER_SIZE.z));
    let boulder_material = materials.add(Color::srgb(0.18, 0.17, 0.15));
    for index in -BOULDER_HALF_COUNT..=BOULDER_HALF_COUNT {
        for side in [-1.0, 1.0] {
            let variation = ((index * 17 + if side > 0.0 { 7 } else { 0 }).rem_euclid(5)) as f32;
            let angle = (variation - 2.0) * 0.09;
            let scale = Vec3::new(1.0 + variation * 0.07, 0.9 + variation * 0.08, 1.0);
            let x_extent = (BOULDER_SIZE.x * scale.x * angle.cos().abs()
                + BOULDER_SIZE.z * scale.z * angle.sin().abs())
                * 0.5;
            commands.spawn((
                Mesh3d(boulder_mesh.clone()),
                MeshMaterial3d(boulder_material.clone()),
                Transform::from_xyz(
                    centerline_x(index as f32 * BOULDER_SPACING)
                        + side * (ROAD_HALF_WIDTH + x_extent),
                    0.65 + variation * 0.06,
                    index as f32 * BOULDER_SPACING,
                )
                .with_rotation(Quat::from_rotation_y(angle))
                .with_scale(scale),
                TerrainVisual,
                Boulder(index),
            ));
        }
    }
}

pub fn follow_route(
    players: Query<&Transform, (With<Player>, With<LocalPlayer>, Without<TerrainVisual>)>,
    mut visuals: Query<(&mut Transform, Option<&Boulder>), With<TerrainVisual>>,
    mut road: ResMut<RoadMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let center_segment = (player.translation.z / BOULDER_SPACING).floor() as i32;
    let mesh_center = center_segment as f32 * BOULDER_SPACING;
    if road.center_segment != center_segment {
        if let Some(mut mesh) = meshes.get_mut(&road.handle) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, road_positions(mesh_center));
            road.center_segment = center_segment;
        }
    }
    for (mut transform, boulder) in &mut visuals {
        if let Some(boulder) = boulder {
            let z = boulder_segment(boulder.0, center_segment) as f32 * BOULDER_SPACING;
            let side = transform.translation.x.signum();
            let angle = ((boulder.0 * 17 + if side > 0.0 { 7 } else { 0 }).rem_euclid(5) as f32
                - 2.0)
                * 0.09
                + centerline_slope(z).atan();
            let x_extent = (BOULDER_SIZE.x * transform.scale.x * angle.cos().abs()
                + BOULDER_SIZE.z * transform.scale.z * angle.sin().abs())
                * 0.5;
            transform.translation.x = centerline_x(z) + side * (ROAD_HALF_WIDTH + x_extent);
            transform.translation.z = z;
            transform.rotation = Quat::from_rotation_y(angle);
        } else if transform.translation.y < 0.0 {
            transform.translation.x = centerline_x(player.translation.z);
            transform.translation.z = player.translation.z;
        } else {
            transform.translation.z = mesh_center;
        }
    }
}

fn boulder_segment(index: i32, center_segment: i32) -> i32 {
    // Visible meshes keep their world position. Only a mesh beyond the pool's
    // range wraps around to the far side of the camera.
    let relative = (index - center_segment + BOULDER_HALF_COUNT).rem_euclid(BOULDER_COUNT)
        - BOULDER_HALF_COUNT;
    center_segment + relative
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearby_boulders_do_not_shift_when_player_crosses_a_segment() {
        for index in -BOULDER_HALF_COUNT + 1..=BOULDER_HALF_COUNT {
            assert_eq!(boulder_segment(index, 0), boulder_segment(index, 1));
        }
        assert_eq!(
            boulder_segment(-BOULDER_HALF_COUNT, 1),
            BOULDER_HALF_COUNT + 1
        );
    }

    #[test]
    fn bends_are_continuous_and_never_steeper_than_fifteen_percent() {
        assert_eq!(centerline_x(0.0), 0.0);
        for step in 0..10_000 {
            let z = -step as f32 * 0.5;
            assert!(centerline_x(z).abs() <= MAX_OFFSET + 0.001);
            assert!(centerline_slope(z).abs() <= 0.15);
            assert!((centerline_x(z) - centerline_x(z - 0.01)).abs() <= 0.0015);
        }
    }

    #[test]
    fn curved_boundaries_and_ground_use_the_same_centerline() {
        let z = -135.0;
        let center = centerline_x(z);
        assert_eq!(
            clamp_actor_x(center + 100.0, z, 0.9),
            center + ROAD_HALF_WIDTH - 0.9
        );
        assert_eq!(
            clamp_actor_x(center - 100.0, z, 0.9),
            center - ROAD_HALF_WIDTH + 0.9
        );
        let vertices = road_positions(z);
        let first_z = z + VISIBLE_LENGTH * 0.5;
        assert_eq!(vertices[0][0], centerline_x(first_z) - ROAD_HALF_WIDTH);
        assert_eq!(vertices[1][0], centerline_x(first_z) + ROAD_HALF_WIDTH);
    }
}
