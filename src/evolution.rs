use std::fmt;

use crate::fitness::{Evaluation, TaskKind, evaluate};
use crate::genome::{Genome, NeuralGenome, controller_input_count};
use crate::policy::ControllerKind;
use crate::qd::{Archive, Axis};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchMode {
    Classic,
    MapElites,
}

#[derive(Clone, Debug)]
pub struct EvolutionConfig {
    pub seed: u64,
    pub population_size: usize,
    pub generations: usize,
    pub evaluation_steps: usize,
    pub task: TaskKind,
    pub controller: Option<ControllerKind>,
    pub search_mode: SearchMode,
    pub archive_width: usize,
    pub archive_height: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            population_size: 32,
            generations: 20,
            evaluation_steps: 240,
            task: TaskKind::RoughTerrain,
            controller: None,
            search_mode: SearchMode::MapElites,
            archive_width: 12,
            archive_height: 8,
        }
    }
}

impl EvolutionConfig {
    pub fn validate(&self) -> Result<(), EvolutionConfigError> {
        if self.population_size == 0 {
            return Err(EvolutionConfigError::new(
                "population must be greater than zero",
            ));
        }
        if self.archive_width == 0 || self.archive_height == 0 {
            return Err(EvolutionConfigError::new(
                "archive width and height must be greater than zero",
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvolutionConfigError {
    message: &'static str,
}

impl EvolutionConfigError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl fmt::Display for EvolutionConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for EvolutionConfigError {}

#[derive(Clone, Debug)]
pub struct EvolutionReport {
    pub best_genome: Genome,
    pub best_evaluation: Evaluation,
    pub archive: Archive,
    pub generations: usize,
}

pub fn run_evolution(config: EvolutionConfig) -> Result<EvolutionReport, EvolutionConfigError> {
    config.validate()?;

    let mut rng = Rng::new(config.seed);
    let mut population: Vec<Genome> = (0..config.population_size)
        .map(|index| {
            let controller = config.controller.unwrap_or_else(|| match index % 3 {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            });
            Genome::minimal(controller, &mut rng)
        })
        .collect();

    let mut archive = Archive::new(
        Axis::StableDistance,
        Axis::BodyCount,
        config.archive_width,
        config.archive_height,
    );
    let mut best_genome = population[0].clone();
    let mut best_evaluation = evaluate(&best_genome, config.task, config.evaluation_steps);

    for _ in 0..config.generations {
        let mut scored = Vec::with_capacity(population.len());
        for genome in population {
            let evaluation = evaluate(&genome, config.task, config.evaluation_steps);
            if evaluation.fitness > best_evaluation.fitness {
                best_genome = genome.clone();
                best_evaluation = evaluation.clone();
            }
            archive.insert(genome.clone(), evaluation.clone());
            scored.push((genome, evaluation));
        }

        scored.sort_by(|a, b| {
            b.1.fitness
                .partial_cmp(&a.1.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let elite_count = (config.population_size / 4).max(1);
        let elites: Vec<Genome> = scored
            .iter()
            .take(elite_count)
            .map(|(genome, _)| genome.clone())
            .collect();

        let mut next_population = elites.clone();
        while next_population.len() < config.population_size {
            let parent = if config.search_mode == SearchMode::MapElites {
                archive
                    .sample_parent(&mut rng)
                    .cloned()
                    .unwrap_or_else(|| elites[rng.range_usize(elites.len())].clone())
            } else {
                elites[rng.range_usize(elites.len())].clone()
            };
            let mut child = parent.mutate(&mut rng);
            if let Some(controller) = config.controller {
                force_controller(&mut child, controller, &mut rng);
            }
            next_population.push(child);
        }

        population = next_population;
    }

    Ok(EvolutionReport {
        best_genome,
        best_evaluation,
        archive,
        generations: config.generations,
    })
}

fn force_controller(genome: &mut Genome, controller: ControllerKind, rng: &mut Rng) {
    let input_count = controller_input_count(&genome.body, controller);
    let output_count = genome.body.output_count();
    if genome.controller == controller
        && genome.brain.input_count == input_count
        && genome.brain.output_count == output_count
    {
        return;
    }

    genome.controller = controller;
    genome.brain = NeuralGenome::minimal(input_count, output_count, rng);
}

#[cfg(test)]
mod tests {
    use crate::policy::ControllerKind;

    use super::{EvolutionConfig, SearchMode, run_evolution};

    #[test]
    fn evolution_produces_archive_and_best_creature() {
        let report = run_evolution(EvolutionConfig {
            population_size: 8,
            generations: 2,
            evaluation_steps: 20,
            search_mode: SearchMode::MapElites,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");

        assert!(report.archive.occupied_count() > 0);
        assert!(report.best_evaluation.fitness.is_finite());
    }

    #[test]
    fn evolution_rejects_zero_population() {
        let error = run_evolution(EvolutionConfig {
            population_size: 0,
            ..EvolutionConfig::default()
        })
        .expect_err("zero population should be rejected");

        assert_eq!(error.to_string(), "population must be greater than zero");
    }

    fn small_run(seed: u64) -> super::EvolutionReport {
        run_evolution(EvolutionConfig {
            seed,
            population_size: 12,
            generations: 4,
            evaluation_steps: 40,
            search_mode: SearchMode::MapElites,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run")
    }

    #[test]
    fn evolution_is_deterministic_for_a_fixed_seed() {
        let a = small_run(2024);
        let b = small_run(2024);

        // Same seed must reproduce the whole observable outcome bit-for-bit:
        // best fitness, the winning creature's behaviour metrics, and the
        // shape of the quality-diversity archive.
        assert_eq!(a.best_evaluation.fitness, b.best_evaluation.fitness);
        assert_eq!(a.best_evaluation.steps, b.best_evaluation.steps);
        assert_eq!(
            a.best_evaluation.metrics.stable_distance,
            b.best_evaluation.metrics.stable_distance
        );
        assert_eq!(a.best_evaluation.metrics.body_count, b.best_evaluation.metrics.body_count);
        assert_eq!(a.archive.occupied_count(), b.archive.occupied_count());
        assert_eq!(a.archive.coverage(), b.archive.coverage());
    }

    #[test]
    fn evolution_diverges_across_seeds() {
        // If the seed were ignored, every run would be identical. Across a
        // spread of seeds at least one observable must differ.
        let baseline = small_run(1);
        let differs = [2u64, 3, 4, 5].iter().any(|&seed| {
            let other = small_run(seed);
            other.best_evaluation.fitness != baseline.best_evaluation.fitness
                || other.archive.occupied_count() != baseline.archive.occupied_count()
        });
        assert!(differs, "changing the seed changed nothing — determinism has collapsed to a constant");
    }

    #[test]
    fn fixed_controller_evolution_keeps_requested_controller() {
        let report = run_evolution(EvolutionConfig {
            controller: Some(ControllerKind::Cpg),
            population_size: 8,
            generations: 2,
            evaluation_steps: 20,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");

        assert_eq!(report.best_genome.controller, ControllerKind::Cpg);
        assert!(
            report
                .archive
                .elites()
                .all(|elite| elite.genome.controller == ControllerKind::Cpg)
        );
    }
}
