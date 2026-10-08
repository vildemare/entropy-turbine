//! The two chamber rings at the lower right of the screen.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};

use crate::player::{GunslingerWeapon, LocalPlayer};
use crate::ui::UiFonts;

const DRUM_CENTER: f32 = 70.0;
const CHAMBER_SIZE: f32 = 19.0;

#[derive(Component)]
pub struct AmmoHud {
    displayed_angle: [f32; 2],
    capacity: [usize; 2],
}

#[derive(Component)]
pub(crate) struct DrumRing(usize);

#[derive(Component)]
pub(crate) struct ChamberIcon {
    gun: usize,
    index: usize,
}

fn ring_diameter(chambers: usize) -> f32 {
    86.0 + chambers.saturating_sub(3) as f32 * 10.0
}

pub fn spawn(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            AmmoHud {
                displayed_angle: [0.0; 2],
                capacity: [3; 2],
            },
            Node {
                position_type: PositionType::Absolute,
                right: px(20),
                bottom: px(52),
                width: px(292),
                height: px(148),
                flex_direction: FlexDirection::Row,
                column_gap: px(12),
                ..default()
            },
            GlobalZIndex(5),
        ))
        .with_children(|hud| {
            for gun in 0..2 {
                hud.spawn(Node {
                    position_type: PositionType::Relative,
                    width: px(140),
                    height: px(148),
                    ..default()
                })
                .with_children(|drum| {
                    drum.spawn((
                        DrumRing(gun),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(27),
                            top: px(27),
                            width: px(86),
                            height: px(86),
                            border: UiRect::all(px(2)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.06, 0.06, 0.055)),
                        BorderColor::all(Color::srgb(0.68, 0.49, 0.26)),
                    ));
                    for index in 0..6 {
                        drum.spawn((
                            ChamberIcon { gun, index },
                            Node {
                                position_type: PositionType::Absolute,
                                width: px(CHAMBER_SIZE),
                                height: px(CHAMBER_SIZE),
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                            BorderColor::all(Color::WHITE),
                            if index < 3 {
                                Visibility::Visible
                            } else {
                                Visibility::Hidden
                            },
                        ));
                    }
                    drum.spawn((
                        Text::new(if gun == 0 { "LEFT" } else { "RIGHT" }),
                        TextFont {
                            font: FontSource::Handle(fonts.body.clone()),
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.72, 0.51)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(42),
                            bottom: px(1),
                            ..default()
                        },
                    ));
                });
            }
        });
}

pub fn update(
    time: Res<Time>,
    weapons: Query<&GunslingerWeapon, With<LocalPlayer>>,
    mut hud: Query<&mut AmmoHud>,
    mut rings: Query<(&DrumRing, &mut Node), Without<ChamberIcon>>,
    mut icons: Query<
        (
            &ChamberIcon,
            &mut Node,
            &mut BackgroundColor,
            &mut Visibility,
        ),
        Without<DrumRing>,
    >,
) {
    let (Some(weapon), Ok(mut hud)) = (weapons.iter().next(), hud.single_mut()) else {
        return;
    };
    for gun in 0..2 {
        let count = weapon.drums[gun].chambers.len();
        let target = weapon.drums[gun].spin_steps as f32 * TAU / count as f32;
        if hud.capacity[gun] != count {
            hud.displayed_angle[gun] = target;
            hud.capacity[gun] = count;
        } else {
            hud.displayed_angle[gun] +=
                (target - hud.displayed_angle[gun]).clamp(0.0, time.delta_secs() * 17.0);
        }
    }
    for (ring, mut node) in &mut rings {
        let diameter = ring_diameter(weapon.drums[ring.0].chambers.len());
        node.width = px(diameter);
        node.height = px(diameter);
        node.left = px(DRUM_CENTER - diameter * 0.5);
        node.top = px(DRUM_CENTER - diameter * 0.5);
    }
    for (icon, mut node, mut background, mut visibility) in &mut icons {
        let drum = &weapon.drums[icon.gun];
        if icon.index >= drum.chambers.len() {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let count = drum.chambers.len();
        let orbit = ring_diameter(count) * 0.5 - 13.0;
        let angle =
            icon.index as f32 * TAU / count as f32 - hud.displayed_angle[icon.gun] - FRAC_PI_2;
        node.left = px(DRUM_CENTER + orbit * angle.cos() - CHAMBER_SIZE * 0.5);
        node.top = px(DRUM_CENTER + orbit * angle.sin() - CHAMBER_SIZE * 0.5);
        *background = BackgroundColor(if drum.chambers[icon.index] {
            Color::WHITE
        } else {
            Color::BLACK
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn chamber_ring_grows_with_capacity() {
        assert!(ring_diameter(4) > ring_diameter(3));
        assert!(ring_diameter(6) > ring_diameter(5));
        assert!(ring_diameter(6) < 140.0);
    }

    #[test]
    fn fired_chamber_turns_black_and_next_chamber_moves_to_top() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Startup, |mut commands: Commands| {
                spawn(
                    &mut commands,
                    &UiFonts {
                        heading: Handle::default(),
                        body: Handle::default(),
                    },
                );
            })
            .add_systems(Update, update);
        let player = app
            .world_mut()
            .spawn((LocalPlayer, GunslingerWeapon::default()))
            .id();
        app.update();
        {
            let mut weapon = app.world_mut().get_mut::<GunslingerWeapon>(player).unwrap();
            weapon.drums[0].chambers[0] = false;
            weapon.drums[0].next = 1;
            weapon.drums[0].spin_steps = 1;
            weapon.rounds -= 1;
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(200));
        app.update();
        let mut query = app
            .world_mut()
            .query::<(&ChamberIcon, &Node, &BackgroundColor, &BorderColor)>();
        let mut empty = None;
        let mut next = None;
        for (icon, node, fill, border) in query.iter(app.world()) {
            if icon.gun == 0 && icon.index == 0 {
                empty = Some((fill.0, border.top));
            }
            if icon.gun == 0 && icon.index == 1 {
                next = Some(node.top);
            }
        }
        assert_eq!(empty, Some((Color::BLACK, Color::WHITE)));
        assert_eq!(next, Some(px(30.5)));
    }
}
