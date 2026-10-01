use bevy::prelude::*;

use crate::player::{LocalPlayer, Player};

pub const ROAD_HALF_WIDTH: f32 = 13.0;
const VISIBLE_LENGTH: f32 = 300.0;
const DASH_SPACING: f32 = 8.0;
const POST_SPACING: f32 = 16.0;

#[derive(Component)]
pub(crate) struct StreetVisual;

#[derive(Component)]
pub(crate) struct LaneDash(i32);

#[derive(Component)]
pub(crate) struct WallPost(i32);

/// Keep an actor's center far enough from the street edge for its body to fit.
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
        MeshMaterial3d(materials.add(Color::srgb(0.035, 0.037, 0.039))),
        Transform::from_xyz(0.0, -0.04, 0.0),
        StreetVisual,
    ));
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(ROAD_HALF_WIDTH * 2.0, VISIBLE_LENGTH),
            ),
        ),
        MeshMaterial3d(materials.add(Color::srgb(0.095, 0.105, 0.107))),
        Transform::from_xyz(0.0, 0.0, 0.0),
        StreetVisual,
    ));

    let edge_mesh = meshes.add(Cuboid::new(0.18, 0.05, VISIBLE_LENGTH));
    let edge_material = materials.add(Color::srgb(0.68, 0.41, 0.13));
    let wall_mesh = meshes.add(Cuboid::new(0.5, 0.8, VISIBLE_LENGTH));
    let wall_material = materials.add(Color::srgb(0.19, 0.16, 0.14));
    for side in [-1.0, 1.0] {
        commands.spawn((
            Mesh3d(edge_mesh.clone()),
            MeshMaterial3d(edge_material.clone()),
            Transform::from_xyz(side * (ROAD_HALF_WIDTH - 0.09), 0.035, 0.0),
            StreetVisual,
        ));
        commands.spawn((
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_xyz(side * (ROAD_HALF_WIDTH + 0.25), 0.4, 0.0),
            StreetVisual,
        ));
    }

    let dash_mesh = meshes.add(Cuboid::new(0.12, 0.025, 2.8));
    let dash_material = materials.add(Color::srgb(0.36, 0.31, 0.23));
    for index in -18..=18 {
        commands.spawn((
            Mesh3d(dash_mesh.clone()),
            MeshMaterial3d(dash_material.clone()),
            Transform::from_xyz(0.0, 0.02, index as f32 * DASH_SPACING),
            StreetVisual,
            LaneDash(index),
        ));
    }

    let post_mesh = meshes.add(Cuboid::new(0.8, 1.2, 0.8));
    let cap_mesh = meshes.add(Cuboid::new(0.95, 0.09, 0.95));
    let post_material = materials.add(Color::srgb(0.12, 0.13, 0.13));
    let cap_material = materials.add(Color::srgb(0.52, 0.30, 0.10));
    for index in -9..=9 {
        for side in [-1.0, 1.0] {
            let x = side * (ROAD_HALF_WIDTH + 0.35);
            let z = index as f32 * POST_SPACING;
            commands.spawn((
                Mesh3d(post_mesh.clone()),
                MeshMaterial3d(post_material.clone()),
                Transform::from_xyz(x, 0.6, z),
                StreetVisual,
                WallPost(index),
            ));
            commands.spawn((
                Mesh3d(cap_mesh.clone()),
                MeshMaterial3d(cap_material.clone()),
                Transform::from_xyz(x, 1.23, z),
                StreetVisual,
                WallPost(index),
            ));
        }
    }
}

pub fn follow_street(
    players: Query<&Transform, (With<Player>, With<LocalPlayer>, Without<StreetVisual>)>,
    mut visuals: Query<(&mut Transform, Option<&LaneDash>, Option<&WallPost>), With<StreetVisual>>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let dash_anchor = (player.translation.z / DASH_SPACING).floor() * DASH_SPACING;
    let post_anchor = (player.translation.z / POST_SPACING).floor() * POST_SPACING;
    for (mut transform, dash, post) in &mut visuals {
        transform.translation.z = if let Some(dash) = dash {
            dash_anchor + dash.0 as f32 * DASH_SPACING
        } else if let Some(post) = post {
            post_anchor + post.0 as f32 * POST_SPACING
        } else {
            player.translation.z
        };
    }
}
