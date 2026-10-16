//! Faint blue shell around a body whose energy shield is up.
//!
//! The shell is its own mesh, so the shared body materials stay as they are.
//! While the shield is down the shell hides, and it returns with the pool.
//! An absorbed hit briefly adds a brighter, swelling shell.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::ability::EnergyShield;
use crate::enemy::{Enemy, Grenadier, ZapBot};
use crate::player::Player;

const SHADER: &str = "shaders/shield_shimmer.wesl";
const HIT_FLASH_SECONDS: f32 = 0.18;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShieldMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub hit: f32,
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
    hit_material: Handle<ShieldMaterial>,
}

#[derive(Component)]
pub(crate) struct ShieldShell;

#[derive(Component)]
pub(crate) struct ShieldHitFlash {
    age: f32,
    radius: f32,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
) {
    commands.insert_resource(ShieldAssets {
        mesh: meshes.add(Sphere::new(1.0)),
        material: materials.add(ShieldMaterial {
            color: LinearRgba::new(0.45, 0.78, 1.0, 1.0),
            hit: 0.0,
        }),
        hit_material: materials.add(ShieldMaterial {
            color: LinearRgba::new(0.45, 0.78, 1.0, 1.0),
            hit: 1.0,
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

pub fn flash_hits(
    time: Res<Time>,
    mut notices: MessageReader<crate::ability::Notice>,
    mut commands: Commands,
    assets: Res<ShieldAssets>,
    bodies: Query<(
        Option<&Enemy>,
        Option<&ZapBot>,
        Option<&Grenadier>,
        Has<Player>,
    )>,
    mut flashes: Query<(Entity, &ChildOf, &mut ShieldHitFlash, &mut Transform)>,
) {
    let mut hit: Vec<Entity> = notices
        .read()
        .filter_map(|notice| match notice {
            crate::ability::Notice::Shield { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    hit.sort_unstable();
    hit.dedup();

    for (entity, parent, mut flash, mut transform) in &mut flashes {
        if let Ok(index) = hit.binary_search(&parent.parent()) {
            hit.remove(index);
            flash.age = 0.0;
        } else {
            flash.age += time.delta_secs();
        }
        if flash.age >= HIT_FLASH_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = flash.age / HIT_FLASH_SECONDS;
        let swell = 1.0 + (progress * std::f32::consts::PI).sin() * 0.07;
        transform.scale = Vec3::splat(flash.radius * swell);
    }

    for target in hit {
        let Ok((enemy, zap, grenadier, player)) = bodies.get(target) else {
            continue;
        };
        let radius = shell_radius(enemy, zap, grenadier, player);
        commands.spawn((
            ShieldHitFlash { age: 0.0, radius },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.hit_material.clone()),
            Transform::from_scale(Vec3::splat(radius)),
            Visibility::default(),
            ChildOf(target),
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
        return crate::character::gunslinger().hit_radius() * 1.8;
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
            hit_material: Handle::default(),
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

    #[test]
    fn absorbing_a_hit_briefly_adds_a_bright_shield_shell() {
        let mut app = App::new();
        app.add_message::<crate::ability::Notice>()
            .insert_resource(Time::<()>::default())
            .insert_resource(ShieldAssets {
                mesh: Handle::default(),
                material: Handle::default(),
                hit_material: Handle::default(),
            })
            .add_systems(Update, flash_hits);
        let body = app
            .world_mut()
            .spawn((EnergyShield::full(1_600, 4.0), Transform::default()))
            .id();
        app.world_mut()
            .write_message(crate::ability::Notice::Shield {
                target: body,
                broken: false,
            });
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&ShieldHitFlash>()
                .iter(app.world())
                .count(),
            1
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(200));
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&ShieldHitFlash>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
