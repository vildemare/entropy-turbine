//! Shared meshes and materials for actors, shots, cover, and pickups.

use bevy::prelude::*;
use bevy::{asset::RenderAssetUsages, render::render_resource::PrimitiveTopology};

#[derive(Resource, Clone, Default)]
pub struct Visuals {
    pub enemy_part_mesh: Handle<Mesh>,
    pub scout_material: Handle<StandardMaterial>,
    pub trooper_material: Handle<StandardMaterial>,
    pub heavy_material: Handle<StandardMaterial>,
    pub enemy_iron_material: Handle<StandardMaterial>,
    pub enemy_brass_material: Handle<StandardMaterial>,
    pub enemy_visor_material: Handle<StandardMaterial>,
    pub bullet_mesh: Handle<Mesh>,
    pub player_bullet_mesh: Handle<Mesh>,
    pub zap_bolt_mesh: Handle<Mesh>,
    pub grenade_mesh: Handle<Mesh>,
    pub blast_ring_mesh: Handle<Mesh>,
    pub bullet_material: Handle<StandardMaterial>,
    pub enemy_bullet_material: Handle<StandardMaterial>,
    pub evolution_bullet_material: Handle<StandardMaterial>,
    pub zap_bolt_material: Handle<StandardMaterial>,
    pub grenade_material: Handle<StandardMaterial>,
    pub blast_mark_material: Handle<StandardMaterial>,
    pub barricade_mesh: Handle<Mesh>,
    pub barricade_material: Handle<StandardMaterial>,
    pub rubble_scene: Handle<WorldAsset>,
    pub orb_mesh: Handle<Mesh>,
    pub orb_material: Handle<StandardMaterial>,
    pub checkpoint_mesh: Handle<Mesh>,
    pub checkpoint_material: Handle<StandardMaterial>,
}

/// Length of the zap jolt mesh along local +Z. The projectile sits at the tail.
pub const ZAP_BOLT_LENGTH: f32 = 4.0;

pub fn load(
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Visuals {
    Visuals {
        enemy_part_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        scout_material: materials.add(Color::srgb(0.40, 0.22, 0.12)),
        trooper_material: materials.add(Color::srgb(0.24, 0.27, 0.25)),
        heavy_material: materials.add(Color::srgb(0.15, 0.17, 0.19)),
        enemy_iron_material: materials.add(Color::srgb(0.12, 0.13, 0.14)),
        enemy_brass_material: materials.add(Color::srgb(0.58, 0.39, 0.13)),
        enemy_visor_material: materials.add(Color::srgb(0.60, 0.14, 0.07)),
        bullet_mesh: meshes.add(Sphere::new(0.17)),
        player_bullet_mesh: meshes.add(player_bullet_mesh()),
        zap_bolt_mesh: meshes.add(Cuboid::new(1.0, 0.8, ZAP_BOLT_LENGTH)),
        grenade_mesh: meshes.add(Sphere::new(0.16)),
        blast_ring_mesh: meshes.add(Annulus::new(0.86, 1.0)),
        bullet_material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            ..default()
        }),
        enemy_bullet_material: materials.add(Color::srgb(0.88, 0.19, 0.07)),
        evolution_bullet_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.83, 0.12),
            emissive: LinearRgba::new(5.0, 2.6, 0.08, 1.0),
            unlit: true,
            ..default()
        }),
        zap_bolt_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.84, 0.45),
            emissive: LinearRgba::new(6.0, 2.2, 0.35, 1.0),
            unlit: true,
            ..default()
        }),
        grenade_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.12, 0.1),
            emissive: LinearRgba::new(1.6, 0.12, 0.04, 1.0),
            ..default()
        }),
        blast_mark_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.08, 0.04),
            emissive: LinearRgba::new(4.0, 0.18, 0.05, 1.0),
            unlit: true,
            double_sided: true,
            ..default()
        }),
        barricade_mesh: meshes.add(Cuboid::new(5.4, 1.1, 1.0)),
        barricade_material: materials.add(Color::srgb(0.20, 0.17, 0.13)),
        rubble_scene: asset_server
            .load(GltfAssetLabel::Scene(0).from_asset("models/rubble_cluster.glb")),
        orb_mesh: meshes.add(Sphere::new(0.32)),
        orb_material: materials.add(Color::srgb(0.95, 0.65, 0.18)),
        checkpoint_mesh: meshes.add(Cuboid::new(25.0, 0.08, 0.4)),
        checkpoint_material: materials.add(Color::srgb(0.76, 0.55, 0.22)),
    }
}

fn player_bullet_mesh() -> Mesh {
    // The pointed head faces local +Z. A second, translucent triangle fades
    // backwards, so rotating the projectile also rotates its short trail.
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [0.0, 0.0, 0.55],
            [0.23, 0.0, -0.22],
            [-0.23, 0.0, -0.22],
            [-0.16, 0.0, -0.22],
            [0.16, 0.0, -0.22],
            [0.0, 0.0, -1.35],
        ],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 6]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![
            [0.79, 0.88, 0.94, 1.0],
            [0.70, 0.82, 0.91, 0.95],
            [0.70, 0.82, 0.91, 0.95],
            [0.55, 0.72, 0.84, 0.23],
            [0.55, 0.72, 0.84, 0.23],
            [0.55, 0.72, 0.84, 0.0],
        ],
    );
    mesh
}
