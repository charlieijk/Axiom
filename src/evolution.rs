use std::fmt;

use serde::{Deserialize, Serialize};

use crate::fitness::{Evaluation, TaskKind, evaluate};
use crate::genome::{Genome, NeuralGenome, controller_input_count};
use crate::policy::ControllerKind;
use crate::qd::{Archive, Axis, Elite};
use crate::rng::Rng;
use crate::task_pack::{TaskPackKind, evaluate_pack};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchMode {
    Classic,
    MapElites,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvolutionReport {
    pub best_genome: Genome,
    pub best_genome_id: u64,
    pub best_evaluation: Evaluation,
    pub archive: Archive,
    pub generations: usize,
    pub evaluated_count: usize,
    pub history: Vec<GenerationSummary>,
    pub lineage: Vec<LineageRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GenerationSummary {
    pub generation: usize,
    pub best_fitness: f32,
    pub mean_fitness: f32,
    pub archive_coverage: f32,
    pub occupied_cells: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineageRecord {
    pub genome_id: u64,
    pub parent_id: Option<u64>,
    pub generation: usize,
    pub controller: ControllerKind,
    pub body_count: usize,
    pub actuator_count: usize,
    pub mutation_summary: String,
}

#[derive(Clone, Debug)]
struct TrackedGenome {
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    mutation_summary: String,
    genome: Genome,
}

impl TrackedGenome {
    fn from_elite(elite: &Elite) -> Self {
        Self {
            genome_id: elite.genome_id,
            parent_id: elite.parent_id,
            generation: elite.generation,
            mutation_summary: elite.mutation_summary.clone(),
            genome: elite.genome.clone(),
        }
    }
}

pub fn run_evolution(config: EvolutionConfig) -> Result<EvolutionReport, EvolutionConfigError> {
    run_evolution_internal(config, None)
}

pub fn run_evolution_with_pack(
    config: EvolutionConfig,
    pack: TaskPackKind,
) -> Result<EvolutionReport, EvolutionConfigError> {
    run_evolution_internal(config, Some(pack))
}

fn run_evolution_internal(
    config: EvolutionConfig,
    task_pack: Option<TaskPackKind>,
) -> Result<EvolutionReport, EvolutionConfigError> {
    config.validate()?;

    let mut rng = Rng::new(config.seed);
    let mut next_genome_id = 1_u64;
    let mut lineage = Vec::new();
    let mut population: Vec<TrackedGenome> = (0..config.population_size)
        .map(|index| {
            let controller = config.controller.unwrap_or(match index % 3 {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            });
            let genome = Genome::minimal(controller, &mut rng);
            let tracked = TrackedGenome {
                genome_id: next_genome_id,
                parent_id: None,
                generation: 0,
                mutation_summary: "seed genome".to_string(),
                genome,
            };
            next_genome_id += 1;
            lineage.push(lineage_record(&tracked));
            tracked
        })
        .collect();

    let mut archive = Archive::new(
        Axis::StableDistance,
        Axis::BodyCount,
        config.archive_width,
        config.archive_height,
    );
    let mut best_genome = population[0].genome.clone();
    let mut best_genome_id = population[0].genome_id;
    let mut best_evaluation = evaluate_for_config(&best_genome, &config, task_pack);
    let mut history = Vec::with_capacity(config.generations);
    let mut evaluated_count = 0;

    for generation in 0..config.generations {
        let current_population = std::mem::take(&mut population);
        let mut scored = Vec::with_capacity(current_population.len());
        for tracked in current_population {
            let evaluation = evaluate_for_config(&tracked.genome, &config, task_pack);
            evaluated_count += 1;
            if evaluation.fitness > best_evaluation.fitness {
                best_genome = tracked.genome.clone();
                best_genome_id = tracked.genome_id;
                best_evaluation = evaluation.clone();
            }
            archive.insert_tracked(
                tracked.genome.clone(),
                evaluation.clone(),
                tracked.genome_id,
                tracked.parent_id,
                tracked.generation,
                tracked.mutation_summary.clone(),
            );
            scored.push((tracked, evaluation));
        }

        scored.sort_by(|a, b| {
            b.1.fitness
                .partial_cmp(&a.1.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mean_fitness = scored
            .iter()
            .map(|(_, evaluation)| evaluation.fitness)
            .sum::<f32>()
            / scored.len() as f32;
        history.push(GenerationSummary {
            generation: generation + 1,
            best_fitness: best_evaluation.fitness,
            mean_fitness,
            archive_coverage: archive.coverage(),
            occupied_cells: archive.occupied_count(),
        });

        let elite_count = (config.population_size / 4).max(1);
        if generation + 1 == config.generations {
            continue;
        }

        let elites: Vec<TrackedGenome> = scored
            .iter()
            .take(elite_count)
            .map(|(tracked, _)| tracked.clone())
            .collect();

        let mut next_population = elites.clone();
        while next_population.len() < config.population_size {
            let parent = if config.search_mode == SearchMode::MapElites {
                archive
                    .sample_elite(&mut rng)
                    .map(TrackedGenome::from_elite)
                    .unwrap_or_else(|| elites[rng.range_usize(elites.len())].clone())
            } else {
                elites[rng.range_usize(elites.len())].clone()
            };
            let mut child = parent.genome.mutate(&mut rng);
            if let Some(controller) = config.controller {
                force_controller(&mut child, controller, &mut rng);
            }
            let tracked = TrackedGenome {
                genome_id: next_genome_id,
                parent_id: Some(parent.genome_id),
                generation: generation + 1,
                mutation_summary: describe_mutation(&parent.genome, &child),
                genome: child,
            };
            next_genome_id += 1;
            lineage.push(lineage_record(&tracked));
            next_population.push(tracked);
        }

        population = next_population;
    }

    Ok(EvolutionReport {
        best_genome,
        best_genome_id,
        best_evaluation,
        archive,
        generations: config.generations,
        evaluated_count,
        history,
        lineage,
    })
}

fn evaluate_for_config(
    genome: &Genome,
    config: &EvolutionConfig,
    task_pack: Option<TaskPackKind>,
) -> Evaluation {
    match task_pack {
        Some(pack) => evaluate_pack(genome, pack, config.evaluation_steps).as_evaluation(),
        None => evaluate(genome, config.task, config.evaluation_steps),
    }
}

fn lineage_record(tracked: &TrackedGenome) -> LineageRecord {
    LineageRecord {
        genome_id: tracked.genome_id,
        parent_id: tracked.parent_id,
        generation: tracked.generation,
        controller: tracked.genome.controller,
        body_count: tracked.genome.body.body_count(),
        actuator_count: tracked.genome.body.actuator_count(),
        mutation_summary: tracked.mutation_summary.clone(),
    }
}

fn describe_mutation(parent: &Genome, child: &Genome) -> String {
    let mut changes = Vec::new();
    let body_delta = child.body.body_count() as isize - parent.body.body_count() as isize;
    if body_delta > 0 {
        changes.push(format!("added {body_delta} body segment"));
    } else if body_delta < 0 {
        changes.push(format!(
            "removed {} body segment",
            body_delta.unsigned_abs()
        ));
    }

    let resized = parent
        .body
        .nodes
        .iter()
        .zip(&child.body.nodes)
        .filter(|(before, after)| before.size != after.size)
        .count();
    if resized > 0 {
        changes.push(format!("reshaped {resized} segment"));
    }

    let retuned_actuators = parent
        .body
        .nodes
        .iter()
        .zip(&child.body.nodes)
        .filter(|(before, after)| before.actuator_strength != after.actuator_strength)
        .count();
    if retuned_actuators > 0 {
        changes.push(format!("retuned {retuned_actuators} actuator"));
    }

    if parent.controller != child.controller {
        changes.push(format!("switched to {} control", child.controller.as_str()));
    }

    let tuned_weights = parent
        .brain
        .connections
        .iter()
        .zip(&child.brain.connections)
        .filter(|(before, after)| before.weight != after.weight)
        .count();
    if tuned_weights > 0 {
        changes.push(format!("tuned {tuned_weights} neural weight"));
    }

    if changes.is_empty() {
        "inherited without measurable mutation".to_string()
    } else {
        changes.join(", ")
    }
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
    use crate::{TaskPackKind, policy::ControllerKind};

    use super::{EvolutionConfig, SearchMode, run_evolution, run_evolution_with_pack};

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
        assert_eq!(report.history.len(), 2);
        assert_eq!(report.history[0].generation, 1);
        assert!(report.history[1].archive_coverage >= report.history[0].archive_coverage);
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

    #[test]
    fn evolution_can_evaluate_the_rough_inspection_pack() {
        let report = run_evolution_with_pack(
            EvolutionConfig {
                population_size: 5,
                generations: 1,
                evaluation_steps: 20,
                ..EvolutionConfig::default()
            },
            TaskPackKind::RoughInspection,
        )
        .expect("valid pack config should run");

        assert!(report.best_evaluation.fitness.is_finite());
        assert!(report.best_evaluation.steps > 20);
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

        // The complete report includes the champion and its metrics, every
        // archive elite, generation history, and lineage. Comparing it
        // directly prevents a deterministic-but-wrong optimization from
        // hiding behind a few unchanged headline metrics.
        assert_eq!(a, b);
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
        assert!(
            differs,
            "changing the seed changed nothing — determinism has collapsed to a constant"
        );
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

    #[test]
    fn evolution_records_parent_lineage_for_archive_elites() {
        let report = run_evolution(EvolutionConfig {
            seed: 71,
            population_size: 8,
            generations: 3,
            evaluation_steps: 20,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");

        assert!(
            report
                .lineage
                .iter()
                .any(|record| record.parent_id.is_some())
        );
        for elite in report.archive.elites() {
            let record = report
                .lineage
                .iter()
                .find(|record| record.genome_id == elite.genome_id)
                .expect("every archived elite should have a lineage record");
            assert_eq!(record.parent_id, elite.parent_id);
        }
    }
}
