//! The experimental lineage that learns between phases.
//!
//! One genome is fielded with every wave.  The body remains exactly as capable
//! as a scout; only movement preferences and trigger decisions are inherited.

use bevy::prelude::*;

use super::{Enemy, EnemyState};
use crate::ability::Notice;
use crate::combat::{BlastRecovery, Health};

pub const POPULATION: usize = super::WAVES_PER_PHASE as usize;
pub const WEIGHTS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Genome {
    pub weights: [f32; WEIGHTS],
}

impl Genome {
    fn initial(index: usize) -> Self {
        let mut state = 0x6d2b_79f5_u32 ^ (index as u32).wrapping_mul(0x9e37_79b9);
        let mut weights = [0.0; WEIGHTS];
        for weight in &mut weights {
            state = scramble(state);
            *weight = state as f32 / u32::MAX as f32 * 1.2 - 0.6;
        }
        // The first generation has a weak, uncertain trigger. Selection must
        // discover when spending a shot is useful rather than receiving a
        // fully competent firing rule for free.
        weights[0] -= 0.9;
        if index == 0 {
            // One exploratory lineage is intentionally trigger-happy. Without
            // any initial shots there can be no hit signal to select.
            weights[0] = 2.0;
        }
        Self { weights }
    }

    pub fn trigger(&self, distance: f32, sight: f32, clear: bool, health: f32) -> bool {
        let input = [
            1.0,
            1.0 - (distance / sight).clamp(0.0, 1.5),
            if clear { 1.0 } else { -1.0 },
            health.clamp(0.0, 1.0) * 2.0 - 1.0,
        ];
        self.weights[..4]
            .iter()
            .zip(input)
            .map(|(weight, value)| weight * value)
            .sum::<f32>()
            > 0.0
    }

    pub fn tactic(&self) -> super::Tactic {
        if self.weights[8..].iter().sum::<f32>() >= 0.0 {
            super::Tactic::Direct
        } else {
            super::Tactic::Straight
        }
    }

    pub fn side(&self) -> f32 {
        if self.weights[4..8].iter().sum::<f32>() >= 0.0 {
            1.0
        } else {
            -1.0
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Outcome {
    genome: Genome,
    fitness: f32,
}

#[derive(Resource)]
pub struct EvolutionLab {
    pub generation: u32,
    genomes: [Genome; POPULATION],
    outcomes: Vec<Outcome>,
    random_state: u32,
}

impl Default for EvolutionLab {
    fn default() -> Self {
        Self {
            generation: 1,
            genomes: std::array::from_fn(Genome::initial),
            outcomes: Vec::with_capacity(POPULATION),
            random_state: 0x91e1_0da5,
        }
    }
}

impl EvolutionLab {
    pub fn genome_for_wave(&self, wave: u32) -> Genome {
        self.genomes[wave as usize % POPULATION]
    }

    fn record(&mut self, bot: &mut EvolutionBot) {
        if bot.recorded {
            return;
        }
        bot.recorded = true;
        // Hitting a player dominates, while travel and survival still break
        // ties. Taking damage is deliberately costly.
        let fitness = bot.hits as f32 * 1_000.0 + bot.age * 4.0 + bot.distance * 2.0
            - bot.damage_taken as f32 * 0.12;
        self.outcomes.push(Outcome {
            genome: bot.genome,
            fitness,
        });
    }

    pub fn record_survivor(&mut self, bot: &mut EvolutionBot) {
        self.record(bot);
    }

    pub fn advance_generation<'a>(&mut self, bots: impl Iterator<Item = Mut<'a, EvolutionBot>>) {
        for mut bot in bots {
            self.record(&mut bot);
        }
        self.outcomes
            .sort_by(|a, b| b.fitness.total_cmp(&a.fitness));
        let winners: Vec<Genome> = self
            .outcomes
            .iter()
            .take(POPULATION / 2)
            .map(|outcome| outcome.genome)
            .collect();
        let winners = if winners.is_empty() {
            self.genomes[..POPULATION / 2].to_vec()
        } else {
            winners
        };
        let mut next = [winners[0]; POPULATION];
        for index in 0..POPULATION {
            let mut genome = winners[index % winners.len()];
            if index >= POPULATION / 2 {
                for _ in 0..2 {
                    self.random_state = scramble(self.random_state);
                    let neuron = self.random_state as usize % WEIGHTS;
                    self.random_state = scramble(self.random_state);
                    let delta = self.random_state as f32 / u32::MAX as f32 * 0.7 - 0.35;
                    genome.weights[neuron] = (genome.weights[neuron] + delta).clamp(-2.0, 2.0);
                }
            }
            next[index] = genome;
        }
        // Fisher-Yates keeps parents and mutations distributed across waves.
        for index in (1..POPULATION).rev() {
            self.random_state = scramble(self.random_state);
            let other = self.random_state as usize % (index + 1);
            next.swap(index, other);
        }
        self.genomes = next;
        self.outcomes.clear();
        self.generation += 1;
    }
}

fn scramble(mut value: u32) -> u32 {
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    value
}

#[derive(Component, Debug)]
pub struct EvolutionBot {
    pub genome: Genome,
    pub fire_timer: f32,
    age: f32,
    distance: f32,
    hits: u32,
    damage_taken: u32,
    last_position: Vec3,
    last_health: u32,
    recorded: bool,
}

impl EvolutionBot {
    pub fn new(genome: Genome, position: Vec3) -> Self {
        Self {
            genome,
            fire_timer: 0.0,
            age: 0.0,
            distance: 0.0,
            hits: 0,
            damage_taken: 0,
            last_position: position,
            last_health: crate::character::scout().health(),
            recorded: false,
        }
    }

    pub fn health_ratio(&self) -> f32 {
        self.last_health as f32 / crate::character::scout().health() as f32
    }
}

pub fn observe_lineage(
    time: Res<Time>,
    mut lab: ResMut<EvolutionLab>,
    mut bots: Query<(
        &Transform,
        &Health,
        &Enemy,
        Has<BlastRecovery>,
        &mut EvolutionBot,
    )>,
) {
    for (transform, health, enemy, displaced, mut bot) in &mut bots {
        bot.age += time.delta_secs();
        if !displaced {
            bot.distance += transform.translation.distance(bot.last_position);
        }
        bot.last_position = transform.translation;
        bot.damage_taken += bot.last_health.saturating_sub(health.0);
        bot.last_health = health.0;
        if enemy.state == EnemyState::Retreating {
            lab.record(&mut bot);
        }
    }
}

pub fn credit_lineage_hits(mut notices: MessageReader<Notice>, mut bots: Query<&mut EvolutionBot>) {
    for notice in notices.read().copied() {
        let Notice::Hurt { player: true, by } = notice else {
            continue;
        };
        if let Ok(mut bot) = bots.get_mut(by) {
            bot.hits += 1;
        }
    }
}

pub fn record_lineage_deaths(
    mut notices: MessageReader<Notice>,
    mut lab: ResMut<EvolutionLab>,
    mut bots: Query<&mut EvolutionBot>,
) {
    for notice in notices.read().copied() {
        let Notice::Died {
            player: false,
            target,
            ..
        } = notice
        else {
            continue;
        };
        if let Ok(mut bot) = bots.get_mut(target) {
            lab.record(&mut bot);
        }
    }
}

#[derive(Component)]
pub struct GeneDot {
    genome: usize,
    group: usize,
}

pub fn spawn_gene_hud(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(20),
                bottom: px(48),
                height: px(24),
                flex_direction: FlexDirection::Row,
                column_gap: px(5),
                ..default()
            },
            GlobalZIndex(8),
        ))
        .with_children(|row| {
            for genome in 0..POPULATION {
                row.spawn((
                    Node {
                        position_type: PositionType::Relative,
                        width: px(20),
                        height: px(20),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.08, 0.07)),
                    BorderColor::all(Color::srgb(0.52, 0.42, 0.2)),
                ))
                .with_children(|circle| {
                    for group in 0..4 {
                        circle.spawn((
                            GeneDot { genome, group },
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(if group % 2 == 0 { 3 } else { 10 }),
                                top: px(if group < 2 { 3 } else { 10 }),
                                width: px(6),
                                height: px(6),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                });
            }
        });
}

pub fn paint_gene_hud(lab: Res<EvolutionLab>, mut dots: Query<(&GeneDot, &mut BackgroundColor)>) {
    if !lab.is_changed() {
        return;
    }
    for (dot, mut color) in &mut dots {
        let weights = &lab.genomes[dot.genome].weights[dot.group * 3..dot.group * 3 + 3];
        let channel = |weight: f32| ((weight + 2.0) * 0.25).clamp(0.08, 1.0);
        *color = BackgroundColor(Color::srgb(
            channel(weights[0]),
            channel(weights[1]),
            channel(weights[2]),
        ));
    }
}

pub fn reset_lineage_on_restart(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<crate::session::Session>,
    mut lab: ResMut<EvolutionLab>,
) {
    if session.phase == crate::session::Phase::PlayerDead && keys.just_pressed(KeyCode::KeyR) {
        *lab = EvolutionLab::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_keeps_winners_and_mutates_half_the_population() {
        let mut lab = EvolutionLab::default();
        let winner = lab.genomes[7];
        lab.outcomes.push(Outcome {
            genome: winner,
            fitness: 10_000.0,
        });
        lab.advance_generation(std::iter::empty());
        assert_eq!(lab.generation, 2);
        assert!(lab.genomes.contains(&winner));
        assert!(lab.genomes.iter().any(|genome| *genome != winner));
    }

    #[test]
    fn initial_lineage_does_not_receive_a_free_trigger_rule() {
        let lab = EvolutionLab::default();
        let firing = lab
            .genomes
            .iter()
            .filter(|genome| genome.trigger(5.0, 18.0, true, 1.0))
            .count();
        assert!(firing > 0);
        assert!(firing < POPULATION);
    }
}
