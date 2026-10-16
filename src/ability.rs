//! Abilities attached to a body, in the order they handle a hit.
//!
//! A [`Struck`] message names the body a round met, what the round is made of,
//! how it is delivered, and how much damage it carries. [`walk`] runs that
//! body's [`Abilities`] from first to last. An ability may lower the damage,
//! stop the chain, or push a [`Notice`] for what should follow: a hurt, a
//! death, an impact. [`crate::combat::deliver_hits`] sends those notices on,
//! and [`crate::combat::present_notices`] plays them out. A reduction sends
//! [`Notice::Armor`] and a live shield sends [`Notice::Shield`] when it spends
//! part of the hit, so those layers can sound different from a body hit.
//!
//! A destructible body carries [`Ability::HitPoints`] or [`Ability::PlayerLife`]
//! together with a [`Health`] value. [`Ability::ReduceDamage`] and
//! [`Ability::EnergyShield`] sit ahead of those when the body should soak part
//! of the hit first.

use bevy::prelude::*;

use crate::session::{Phase, Session};

/// How long a landed hit keeps the player from taking another.
pub const HIT_GRACE: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Player,
    Enemy,
}

/// What the round is made of. Abilities can read this and pass the hit on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Metal,
    Plasma,
    Electricity,
    /// A grenade blast. It reaches every body in the circle.
    Explosive,
}

/// How the round delivers its damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Kinetic,
    Energy,
}

/// Hit points on the shared combat scale where player starting health is 1000.
#[derive(Component)]
pub struct Health(pub u32);

#[derive(Component, Default)]
pub struct HitCooldown(pub f32);

/// A pool on the shared health/damage scale that spends itself against a hit,
/// then stays down for `down_for` seconds.
#[derive(Component, Clone, Copy, Debug)]
pub struct EnergyShield {
    pub remaining: u32,
    pub capacity: u32,
    pub down: f32,
    pub down_for: f32,
}

impl EnergyShield {
    pub fn full(capacity: u32, down_for: f32) -> Self {
        Self {
            remaining: capacity,
            capacity,
            down: 0.0,
            down_for,
        }
    }

    /// The pool is holding and the shimmer should show.
    pub fn is_up(&self) -> bool {
        self.down <= 0.0 && self.remaining > 0
    }
}

/// One step on a body. Earlier steps see the hit before later ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    /// An enemy gunshot stops here. A blast keeps going.
    StopAlliedShots,
    /// A hit during [`HitCooldown`] stops here.
    HitGrace,
    /// After a hit lands, start [`HitCooldown`].
    ArmGrace,
    /// Subtract a flat amount, then pass what remains.
    ReduceDamage(u32),
    /// Spend [`EnergyShield`], then pass the overflow. A collapsed shield passes the hit through.
    EnergyShield,
    /// [`Health`] on an enemy. At zero the body dies and leaves an orb.
    HitPoints,
    /// [`Health`] on the player. At zero the run ends.
    PlayerLife,
    /// The round stops. Asks for the impact sound.
    Solid,
}

#[derive(Component, Clone, Debug)]
pub struct Abilities(pub Vec<Ability>);

impl Abilities {
    pub fn enemy() -> Self {
        Self(vec![Ability::StopAlliedShots, Ability::HitPoints])
    }

    pub fn player() -> Self {
        Self(vec![
            Ability::HitGrace,
            Ability::PlayerLife,
            Ability::ArmGrace,
        ])
    }

    pub fn solid() -> Self {
        Self(vec![Ability::Solid])
    }
}

/// A round met `target`. Handlers on that body decide what follows.
#[derive(Message, Clone, Copy, Debug)]
pub struct Struck {
    pub target: Entity,
    pub source: Entity,
    pub damage: u32,
    pub material: Material,
    pub delivery: Delivery,
    pub faction: Faction,
}

/// What an ability asks a later system to carry out.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum Notice {
    Hurt {
        player: bool,
        by: Entity,
    },
    Died {
        player: bool,
        target: Entity,
        at: Vec3,
    },
    Impact,
    FriendlyStop,
    /// A flat reduction kept some of this hit.
    Armor,
    /// A live shield spent some of its pool on this hit.
    Shield {
        target: Entity,
        broken: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OpenHit {
    pub damage: u32,
    pub material: Material,
    pub delivery: Delivery,
    pub source: Entity,
    pub faction: Faction,
    pub at: Vec3,
    pub consumed: bool,
    pub landed: bool,
}

impl OpenHit {
    fn new(strike: Struck, at: Vec3) -> Self {
        Self {
            damage: strike.damage,
            material: strike.material,
            delivery: strike.delivery,
            source: strike.source,
            faction: strike.faction,
            at,
            consumed: false,
            landed: false,
        }
    }
}

pub(crate) fn walk(
    strike: Struck,
    at: Vec3,
    abilities: &[Ability],
    health: &mut Option<Mut<Health>>,
    grace: &mut Option<Mut<HitCooldown>>,
    shield: &mut Option<Mut<EnergyShield>>,
    notices: &mut Vec<Notice>,
) {
    let mut hit = OpenHit::new(strike, at);
    for ability in abilities.iter().copied() {
        if hit.consumed {
            break;
        }
        handle(
            ability,
            strike.target,
            health,
            grace,
            shield,
            &mut hit,
            notices,
        );
    }
}

fn handle(
    ability: Ability,
    entity: Entity,
    health: &mut Option<Mut<Health>>,
    grace: &mut Option<Mut<HitCooldown>>,
    shield: &mut Option<Mut<EnergyShield>>,
    hit: &mut OpenHit,
    notices: &mut Vec<Notice>,
) {
    match ability {
        Ability::StopAlliedShots => {
            if hit.faction == Faction::Enemy && hit.material != Material::Explosive {
                notices.push(Notice::FriendlyStop);
                hit.consumed = true;
            }
        }
        Ability::HitGrace => {
            if grace.as_ref().is_some_and(|grace| grace.0 > 0.0) {
                hit.consumed = true;
            }
        }
        Ability::ArmGrace => {
            if hit.landed {
                if let Some(grace) = grace.as_mut() {
                    grace.0 = HIT_GRACE;
                }
            }
        }
        Ability::ReduceDamage(amount) => {
            let taken = hit.damage.min(amount);
            if taken > 0 {
                notices.push(Notice::Armor);
            }
            hit.damage -= taken;
            if hit.damage == 0 {
                hit.consumed = true;
            }
        }
        Ability::EnergyShield => absorb_with_shield(entity, shield, hit, notices),
        Ability::HitPoints => take_health(entity, false, health, hit, notices),
        Ability::PlayerLife => take_health(entity, true, health, hit, notices),
        Ability::Solid => {
            notices.push(Notice::Impact);
            hit.consumed = true;
        }
    }
}

fn absorb_with_shield(
    entity: Entity,
    shield: &mut Option<Mut<EnergyShield>>,
    hit: &mut OpenHit,
    notices: &mut Vec<Notice>,
) {
    let Some(shield) = shield.as_mut() else {
        return;
    };
    if shield.down > 0.0 || shield.remaining == 0 {
        return;
    }
    let before = shield.remaining;
    let absorbed = hit.damage.min(before);
    if absorbed > 0 {
        notices.push(Notice::Shield {
            target: entity,
            broken: absorbed == before,
        });
    }
    shield.remaining -= absorbed;
    hit.damage -= absorbed;
    if shield.remaining == 0 {
        shield.down = shield.down_for;
    }
    if hit.damage == 0 {
        hit.consumed = true;
    }
}

fn take_health(
    entity: Entity,
    player: bool,
    health: &mut Option<Mut<Health>>,
    hit: &mut OpenHit,
    notices: &mut Vec<Notice>,
) {
    let Some(health) = health.as_mut() else {
        return;
    };
    if health.0 == 0 || hit.damage == 0 {
        hit.consumed = true;
        return;
    }
    health.0 = health.0.saturating_sub(hit.damage);
    hit.landed = true;
    notices.push(Notice::Hurt {
        player,
        by: hit.source,
    });
    if health.0 == 0 {
        notices.push(Notice::Died {
            player,
            target: entity,
            at: hit.at,
        });
    }
}

pub fn recover_shields(
    time: Res<Time>,
    session: Res<Session>,
    mut shields: Query<&mut EnergyShield>,
) {
    if session.phase != Phase::Playing {
        return;
    }
    let step = time.delta_secs();
    for mut shield in &mut shields {
        if shield.down <= 0.0 {
            continue;
        }
        shield.down = (shield.down - step).max(0.0);
        if shield.down == 0.0 {
            shield.remaining = shield.capacity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_all(
        mut strikes: MessageReader<Struck>,
        mut bodies: Query<(
            &Abilities,
            Option<&mut Health>,
            Option<&mut HitCooldown>,
            Option<&mut EnergyShield>,
            &Transform,
        )>,
    ) {
        for strike in strikes.read().copied() {
            let Ok((abilities, mut health, mut grace, mut shield, transform)) =
                bodies.get_mut(strike.target)
            else {
                continue;
            };
            let mut notices = Vec::new();
            walk(
                strike,
                transform.translation,
                &abilities.0,
                &mut health,
                &mut grace,
                &mut shield,
                &mut notices,
            );
        }
    }

    fn strike(target: Entity, damage: u32) -> Struck {
        Struck {
            target,
            source: Entity::PLACEHOLDER,
            damage,
            material: Material::Plasma,
            delivery: Delivery::Energy,
            faction: Faction::Player,
        }
    }

    #[test]
    fn a_struck_message_keeps_the_rounds_material_and_delivery() {
        let hit = OpenHit::new(
            Struck {
                target: Entity::PLACEHOLDER,
                source: Entity::PLACEHOLDER,
                damage: 200,
                material: Material::Metal,
                delivery: Delivery::Kinetic,
                faction: Faction::Enemy,
            },
            Vec3::new(1.0, 0.0, -3.0),
        );
        assert_eq!(hit.material, Material::Metal);
        assert_eq!(hit.delivery, Delivery::Kinetic);
        assert_eq!(hit.damage, 200);
        assert_eq!(hit.at, Vec3::new(1.0, 0.0, -3.0));
    }

    #[test]
    fn shield_then_flat_reduction_then_hit_points() {
        let mut app = App::new();
        app.add_message::<Struck>()
            .insert_resource(Session::default())
            .insert_resource(Time::<()>::default())
            .add_systems(Update, (apply_all, recover_shields).chain());
        let body = app
            .world_mut()
            .spawn((
                Abilities(vec![
                    Ability::EnergyShield,
                    Ability::ReduceDamage(50),
                    Ability::HitPoints,
                ]),
                EnergyShield::full(500, 1.0),
                Health(1000),
                Transform::from_xyz(0.0, 0.5, 0.0),
            ))
            .id();

        // 600: the shield keeps 500, the flat cut keeps 50, hit points take 50.
        app.world_mut().write_message(strike(body, 600));
        app.update();
        assert_eq!(app.world().get::<Health>(body).unwrap().0, 950);
        let shield = app.world().get::<EnergyShield>(body).unwrap();
        assert_eq!(shield.remaining, 0);
        assert_eq!(shield.down, 1.0);

        // Collapsed shield passes the hit. The flat cut keeps 50 of 80.
        app.world_mut().write_message(strike(body, 80));
        app.update();
        assert_eq!(app.world().get::<Health>(body).unwrap().0, 920);
        assert_eq!(app.world().get::<EnergyShield>(body).unwrap().remaining, 0);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        let shield = app.world().get::<EnergyShield>(body).unwrap();
        assert_eq!(shield.remaining, 500);
        assert_eq!(shield.down, 0.0);

        app.world_mut().write_message(strike(body, 80));
        app.update();
        assert_eq!(app.world().get::<Health>(body).unwrap().0, 920);
        assert_eq!(
            app.world().get::<EnergyShield>(body).unwrap().remaining,
            420
        );
    }

    #[test]
    fn flat_reduction_that_covers_the_hit_leaves_health_unchanged() {
        let mut app = App::new();
        app.add_message::<Struck>().add_systems(Update, apply_all);
        let body = app
            .world_mut()
            .spawn((
                Abilities(vec![Ability::ReduceDamage(50), Ability::HitPoints]),
                Health(100),
                Transform::default(),
            ))
            .id();
        app.world_mut().write_message(strike(body, 50));
        app.update();
        assert_eq!(app.world().get::<Health>(body).unwrap().0, 100);
    }

    #[derive(Resource, Default)]
    struct Heard(Vec<Notice>);

    fn apply_heard(
        mut strikes: MessageReader<Struck>,
        mut heard: ResMut<Heard>,
        mut bodies: Query<(
            &Abilities,
            Option<&mut Health>,
            Option<&mut HitCooldown>,
            Option<&mut EnergyShield>,
            &Transform,
        )>,
    ) {
        for strike in strikes.read().copied() {
            let Ok((abilities, mut health, mut grace, mut shield, transform)) =
                bodies.get_mut(strike.target)
            else {
                continue;
            };
            let mut notices = Vec::new();
            walk(
                strike,
                transform.translation,
                &abilities.0,
                &mut health,
                &mut grace,
                &mut shield,
                &mut notices,
            );
            heard.0.extend(notices);
        }
    }

    #[test]
    fn a_layer_that_takes_damage_announces_itself() {
        let mut app = App::new();
        app.add_message::<Struck>()
            .init_resource::<Heard>()
            .add_systems(Update, apply_heard);
        let armored = app
            .world_mut()
            .spawn((
                Abilities(vec![Ability::ReduceDamage(40), Ability::HitPoints]),
                Health(100),
                Transform::default(),
            ))
            .id();
        app.world_mut().write_message(strike(armored, 100));
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![
                Notice::Armor,
                Notice::Hurt {
                    player: false,
                    by: Entity::PLACEHOLDER,
                },
            ]
        );

        app.world_mut().resource_mut::<Heard>().0.clear();
        let shielded = app
            .world_mut()
            .spawn((
                Abilities(vec![Ability::EnergyShield, Ability::HitPoints]),
                EnergyShield::full(500, 1.0),
                Health(100),
                Transform::default(),
            ))
            .id();
        app.world_mut().write_message(strike(shielded, 80));
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![Notice::Shield {
                target: shielded,
                broken: false,
            }]
        );
        assert_eq!(app.world().get::<Health>(shielded).unwrap().0, 100);

        app.world_mut().resource_mut::<Heard>().0.clear();
        app.world_mut().write_message(strike(shielded, 420));
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![Notice::Shield {
                target: shielded,
                broken: true,
            }]
        );
        let shield = app.world().get::<EnergyShield>(shielded).unwrap();
        assert_eq!(shield.remaining, 0);
        assert!(shield.down > 0.0);

        app.world_mut().resource_mut::<Heard>().0.clear();
        {
            let mut shield = app.world_mut().get_mut::<EnergyShield>(shielded).unwrap();
            shield.remaining = 0;
            shield.down = 1.0;
        }
        app.world_mut().write_message(strike(shielded, 80));
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![Notice::Hurt {
                player: false,
                by: Entity::PLACEHOLDER,
            }]
        );
        assert_eq!(app.world().get::<Health>(shielded).unwrap().0, 20);
    }

    #[test]
    fn a_blast_reaches_an_ally_and_a_gunshot_stops() {
        let mut app = App::new();
        app.add_message::<Struck>()
            .init_resource::<Heard>()
            .add_systems(Update, apply_heard);
        let ally = app
            .world_mut()
            .spawn((Abilities::enemy(), Health(10), Transform::default()))
            .id();
        app.world_mut().write_message(Struck {
            target: ally,
            source: Entity::PLACEHOLDER,
            damage: 4,
            material: Material::Metal,
            delivery: Delivery::Kinetic,
            faction: Faction::Enemy,
        });
        app.update();
        assert_eq!(app.world().get::<Health>(ally).unwrap().0, 10);
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![Notice::FriendlyStop]
        );

        app.world_mut().resource_mut::<Heard>().0.clear();
        app.world_mut().write_message(Struck {
            target: ally,
            source: Entity::PLACEHOLDER,
            damage: 4,
            material: Material::Explosive,
            delivery: Delivery::Kinetic,
            faction: Faction::Enemy,
        });
        app.update();
        assert_eq!(app.world().get::<Health>(ally).unwrap().0, 6);
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![Notice::Hurt {
                player: false,
                by: Entity::PLACEHOLDER,
            }]
        );
    }
}
