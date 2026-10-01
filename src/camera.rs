use bevy::prelude::*;

use crate::player::{LocalPlayer, Player};

#[derive(Component)]
pub struct FollowCamera;

pub fn follow_player(
    time: Res<Time>,
    players: Query<&Transform, (With<Player>, With<LocalPlayer>)>,
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<Player>)>,
) {
    let Some(player) = players.iter().next() else {
        return;
    };
    let target = Vec3::new(0.0, 24.0, player.translation.z + 21.0);
    for mut camera in &mut cameras {
        camera.translation = camera
            .translation
            .lerp(target, 1.0 - (-6.0 * time.delta_secs()).exp());
    }
}
