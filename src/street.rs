use bevy::prelude::*;

use crate::player::{LocalPlayer, Player};

pub const ROAD_HALF_WIDTH: f32 = 13.0;
pub(crate) const BOULDER_SIZE: Vec3 = Vec3::new(2.8, 1.5, 3.5);
const VISIBLE_LENGTH: f32 = 300.0;
const BOULDER_SPACING: f32 = 3.0;
const BOULDER_HALF_COUNT: i32 = 48;
const BOULDER_COUNT: i32 = BOULDER_HALF_COUNT * 2 + 1;

#[derive(Component)]
pub(crate) struct TerrainVisual;

#[derive(Component)]
pub(crate) struct Boulder(pub i32);

/// Keep an actor's center inside the boulder lined playfield.
pub fn clamp_actor_x(x: f32, radius: f32) -> f32 {
    x.clamp(-ROAD_HALF_WIDTH + radius, ROAD_HALF_WIDTH - radius)
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
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(ROAD_HALF_WIDTH * 2.0, VISIBLE_LENGTH),
            ),
        ),
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
                    side * (ROAD_HALF_WIDTH + x_extent),
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

pub fn follow_street(
    players: Query<&Transform, (With<Player>, With<LocalPlayer>, Without<TerrainVisual>)>,
    mut visuals: Query<(&mut Transform, Option<&Boulder>), With<TerrainVisual>>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let center_segment = (player.translation.z / BOULDER_SPACING).floor() as i32;
    for (mut transform, boulder) in &mut visuals {
        transform.translation.z = match boulder {
            Some(boulder) => boulder_segment(boulder.0, center_segment) as f32 * BOULDER_SPACING,
            None => player.translation.z,
        };
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
}
