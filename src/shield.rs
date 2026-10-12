//! Faint blue shell around a body whose energy shield is up.
//!
//! The shell is its own mesh, so the shared body materials stay as they are.
//! While the shield is down the shell hides, and it returns with the pool.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::ability::EnergyShield;
use crate::enemy::{Enemy, Grenadier, ZapBot};
use crate::player::{GUNSLINGER_PLAYER, Player};
use crate::units::units;

const SHADER: &str = "shaders/shield_shimmer.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShieldMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for ShieldMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_shadows() -> bool {
        false
    }

    fn enable_prepass() -> bool {
        false
    }
}

#[derive(Resource)]
pub(crate) struct ShieldAssets {
    mesh: Handle<Mesh>,
    material: Handle<ShieldMaterial>,
}

#[derive(Component)]
pub(crate) struct ShieldShell;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
) {
    commands.insert_resource(ShieldAssets {
        mesh: meshes.add(Sphere::new(1.0)),
        material: materials.add(ShieldMaterial {
            color: LinearRgba::new(0.45, 0.78, 1.0, 1.0),
        }),
    });
}

pub fn sync_shimmer(
    mut commands: Commands,
    assets: Res<ShieldAssets>,
    shields: Query<(
        Entity,
        &EnergyShield,
        Option<&Enemy>,
        Option<&ZapBot>,
        Option<&Grenadier>,
        Has<Player>,
    )>,
    mut shells: Query<(Entity, &ChildOf, &mut Visibility), With<ShieldShell>>,
) {
    let mut attached = Vec::new();
    for (shell, parent, mut visibility) in &mut shells {
        let parent = parent.parent();
        attached.push(parent);
        let Ok((_, shield, _, _, _, _)) = shields.get(parent) else {
            commands.entity(shell).despawn();
            continue;
        };
        *visibility = if shield.is_up() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (entity, shield, enemy, zap, grenadier, player) in &shields {
        if attached.contains(&entity) || !shield.is_up() {
            continue;
        }
        let radius = shell_radius(enemy, zap, grenadier, player);
        commands.spawn((
            ShieldShell,
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_scale(Vec3::splat(radius)),
            Visibility::default(),
            ChildOf(entity),
        ));
    }
}

fn shell_radius(
    enemy: Option<&Enemy>,
    zap: Option<&ZapBot>,
    grenadier: Option<&Grenadier>,
    player: bool,
) -> f32 {
    if let Some(enemy) = enemy {
        return enemy.radius.max(enemy.height) * 1.35;
    }
    if let Some(zap) = zap {
        return zap.radius * 1.2;
    }
    if let Some(grenadier) = grenadier {
        return grenadier.radius * 1.35;
    }
    if player {
        return units(GUNSLINGER_PLAYER.hit_radius) * 1.8;
    }
    0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_up_shield_grows_a_shell_and_a_down_shield_hides_it() {
        let mut app = App::new();
        app.insert_resource(ShieldAssets {
            mesh: Handle::default(),
            material: Handle::default(),
        })
        .add_systems(Update, sync_shimmer);
        let body = app
            .world_mut()
            .spawn((
                EnergyShield::full(100, 1.0),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        app.update();
        let shell = app
            .world()
            .get::<Children>(body)
            .and_then(|children| children.iter().next())
            .expect("shell");
        assert!(app.world().get::<ShieldShell>(shell).is_some());
        let scale = app.world().get::<Transform>(shell).unwrap().scale.x;
        assert!((scale - 0.9).abs() < 0.001);
        assert_eq!(
            app.world().get::<Visibility>(shell).copied(),
            Some(Visibility::Inherited)
        );

        app.world_mut().get_mut::<EnergyShield>(body).unwrap().down = 1.0;
        app.world_mut()
            .get_mut::<EnergyShield>(body)
            .unwrap()
            .remaining = 0;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(shell).copied(),
            Some(Visibility::Hidden)
        );

        {
            let mut shield = app.world_mut().get_mut::<EnergyShield>(body).unwrap();
            shield.down = 0.0;
            shield.remaining = shield.capacity;
        }
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(shell).copied(),
            Some(Visibility::Inherited)
        );
    }
}
