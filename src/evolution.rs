use std::fmt;

use serde::{Deserialize, Serialize};

use crate::fitness::{Evaluation, TaskKind, evaluate};
use crate::genome::Genome;
use crate::policy::ControllerKind;
use crate::qd::{Archive, Axis, Elite};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchMode {
    Classic,
    MapElites,
}

impl SearchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::MapElites => "map-elites",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "classic" => Some(Self::Classic),
            "map-elites" | "map_elites" | "map" => Some(Self::MapElites),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvolutionConfig {
    pub seed: u64,
    pub population_size: usize,
    pub generations: usize,
    pub evaluation_steps: usize,
    pub task: TaskKind,
    pub search_mode: SearchMode,
    pub archive_width: usize,
    pub archive_height: usize,
    pub archive_x_axis: Axis,
    pub archive_y_axis: Axis,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            population_size: 32,
            generations: 20,
            evaluation_steps: 240,
            task: TaskKind::RoughTerrain,
            search_mode: SearchMode::MapElites,
            archive_width: 12,
            archive_height: 8,
            archive_x_axis: Axis::StableDistance,
            archive_y_axis: Axis::BodyCount,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerationSummary {
    pub generation: usize,
    pub evaluated_count: usize,
    pub best_fitness: f32,
    pub best_distance: f32,
    pub coverage: f32,
    pub occupied_cells: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LineageRecord {
    pub genome_id: u64,
    pub parent_id: Option<u64>,
    pub generation: usize,
    pub controller: ControllerKind,
    pub body_count: usize,
    pub actuator_count: usize,
    pub mutation_summary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvolutionReport {
    pub config: EvolutionConfig,
    pub best_genome: Genome,
    pub best_evaluation: Evaluation,
    pub archive: Archive,
    pub generations: usize,
    pub evaluated_count: usize,
    pub generation_summaries: Vec<GenerationSummary>,
    pub lineage: Vec<LineageRecord>,
}

#[derive(Clone, Debug)]
struct TrackedGenome {
    id: u64,
    parent_id: Option<u64>,
    generation: usize,
    mutation_summary: String,
    genome: Genome,
}

impl TrackedGenome {
    fn from_elite(elite: &Elite) -> Self {
        Self {
            id: elite.genome_id,
            parent_id: elite.parent_id,
            generation: elite.generation,
            mutation_summary: elite.mutation_summary.clone(),
            genome: elite.genome.clone(),
        }
    }
}

pub fn run_evolution(config: EvolutionConfig) -> Result<EvolutionReport, EvolutionConfigError> {
    run_evolution_with_progress(config, |_| {})
}

pub fn run_evolution_with_progress<F>(
    config: EvolutionConfig,
    mut on_generation: F,
) -> Result<EvolutionReport, EvolutionConfigError>
where
    F: FnMut(&GenerationSummary),
{
    config.validate()?;

    let mut rng = Rng::new(config.seed);
    let mut next_genome_id = 1_u64;
    let mut lineage = Vec::new();
    let mut population: Vec<TrackedGenome> = (0..config.population_size)
        .map(|index| {
            let controller = match index % 3 {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            };
            let genome = Genome::minimal(controller, &mut rng);
            let id = next_genome_id;
            next_genome_id += 1;
            lineage.push(lineage_record(id, None, 0, "seed".to_string(), &genome));
            TrackedGenome {
                id,
                parent_id: None,
                generation: 0,
                mutation_summary: "seed".to_string(),
                genome,
            }
        })
        .collect();

    let mut archive = Archive::new(
        config.archive_x_axis,
        config.archive_y_axis,
        config.archive_width,
        config.archive_height,
    );
    let mut best_genome = None;
    let mut best_evaluation = None;
    let mut evaluated_count = 0_usize;
    let mut generation_summaries = Vec::with_capacity(config.generations.max(1));

    for generation in 0..config.generations {
        let mut scored = Vec::with_capacity(population.len());
        for tracked in population.drain(..) {
            let evaluation = evaluate(&tracked.genome, config.task, config.evaluation_steps);
            evaluated_count += 1;
            if best_evaluation
                .as_ref()
                .map(|best: &Evaluation| evaluation.fitness > best.fitness)
                .unwrap_or(true)
            {
                best_genome = Some(tracked.genome.clone());
                best_evaluation = Some(evaluation.clone());
            }
            archive.insert_tracked(
                tracked.genome.clone(),
                evaluation.clone(),
                tracked.id,
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

        let summary = generation_summary(
            generation,
            evaluated_count,
            best_evaluation
                .as_ref()
                .expect("each generation evaluates at least one genome"),
            &archive,
        );
        on_generation(&summary);
        generation_summaries.push(summary);

        if generation + 1 == config.generations {
            continue;
        }

        let elite_count = (config.population_size / 4).max(1);
        let elites: Vec<TrackedGenome> = scored
            .iter()
            .take(elite_count)
            .map(|(genome, _)| genome.clone())
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
            let genome = parent.genome.mutate(&mut rng);
            let id = next_genome_id;
            next_genome_id += 1;
            let mutation_summary = mutation_summary(&parent.genome, &genome);
            lineage.push(lineage_record(
                id,
                Some(parent.id),
                generation + 1,
                mutation_summary.clone(),
                &genome,
            ));
            next_population.push(TrackedGenome {
                id,
                parent_id: Some(parent.id),
                generation: generation + 1,
                mutation_summary,
                genome,
            });
        }

        population = next_population;
    }

    if best_evaluation.is_none() {
        let tracked = population
            .first()
            .expect("population has already been validated")
            .clone();
        let evaluation = evaluate(&tracked.genome, config.task, config.evaluation_steps);
        evaluated_count += 1;
        archive.insert_tracked(
            tracked.genome.clone(),
            evaluation.clone(),
            tracked.id,
            tracked.parent_id,
            tracked.generation,
            tracked.mutation_summary.clone(),
        );
        let summary = generation_summary(0, evaluated_count, &evaluation, &archive);
        on_generation(&summary);
        generation_summaries.push(summary);
        best_genome = Some(tracked.genome);
        best_evaluation = Some(evaluation);
    }

    Ok(EvolutionReport {
        config: config.clone(),
        best_genome: best_genome.expect("valid runs evaluate at least one genome"),
        best_evaluation: best_evaluation.expect("valid runs evaluate at least one genome"),
        archive,
        generations: config.generations,
        evaluated_count,
        generation_summaries,
        lineage,
    })
}

fn lineage_record(
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    mutation_summary: String,
    genome: &Genome,
) -> LineageRecord {
    LineageRecord {
        genome_id,
        parent_id,
        generation,
        controller: genome.controller,
        body_count: genome.body.body_count(),
        actuator_count: genome.body.actuator_count(),
        mutation_summary,
    }
}

fn mutation_summary(parent: &Genome, child: &Genome) -> String {
    let body_delta = child.body.body_count() as isize - parent.body.body_count() as isize;
    let actuator_delta =
        child.body.actuator_count() as isize - parent.body.actuator_count() as isize;
    let controller = if child.controller == parent.controller {
        child.controller.as_str().to_string()
    } else {
        format!(
            "{} -> {}",
            parent.controller.as_str(),
            child.controller.as_str()
        )
    };
    format!(
        "controller {controller}; body {body_delta:+}; actuators {actuator_delta:+}; weights mutated"
    )
}

fn generation_summary(
    generation: usize,
    evaluated_count: usize,
    best_evaluation: &Evaluation,
    archive: &Archive,
) -> GenerationSummary {
    GenerationSummary {
        generation,
        evaluated_count,
        best_fitness: best_evaluation.fitness,
        best_distance: best_evaluation.metrics.distance,
        coverage: archive.coverage(),
        occupied_cells: archive.occupied_count(),
    }
}

#[cfg(test)]
mod tests {
    use super::{EvolutionConfig, SearchMode, run_evolution};
    use crate::{TaskKind, qd::Axis};

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
        assert_eq!(report.evaluated_count, 16);
        assert!(!report.lineage.is_empty());
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
    fn evolution_is_deterministic_for_fixed_config() {
        let config = EvolutionConfig {
            seed: 77,
            population_size: 10,
            generations: 3,
            evaluation_steps: 35,
            task: TaskKind::RoughTerrain,
            ..EvolutionConfig::default()
        };

        let first = run_evolution(config.clone()).expect("first run should pass");
        let second = run_evolution(config).expect("second run should pass");

        assert_eq!(first.evaluated_count, second.evaluated_count);
        assert_eq!(
            first.archive.occupied_count(),
            second.archive.occupied_count()
        );
        assert_eq!(first.lineage.len(), second.lineage.len());
        assert!((first.best_evaluation.fitness - second.best_evaluation.fitness).abs() < 0.0001);
    }

    #[test]
    fn evolution_config_controls_archive_axes() {
        let report = run_evolution(EvolutionConfig {
            population_size: 6,
            generations: 1,
            evaluation_steps: 10,
            archive_x_axis: Axis::Distance,
            archive_y_axis: Axis::Stability,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");

        assert_eq!(report.archive.x_axis, Axis::Distance);
        assert_eq!(report.archive.y_axis, Axis::Stability);
    }
}
