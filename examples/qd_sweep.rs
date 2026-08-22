//! Headless quality-diversity harness: runs the real MAP-Elites search and
//! emits one JSON document on stdout.
//!
//! ```text
//! cargo run --release --example qd_sweep -- --seeds 1,2,3 --generations 40
//! ```
//!
//! # Why the trajectory is measured with prefix runs
//!
//! [`axiom::run_evolution`] reports `archive_coverage` per generation but only
//! the *final* archive, so the QD-score trajectory cannot be read out of a
//! single report. Rather than add a field to `EvolutionReport` — whose
//! serialization is pinned by the checked-in Arm64 evidence digest — this
//! harness re-runs the same seed with `generations = 1..=N` and reads the
//! archive at each length.
//!
//! That is exact, not an approximation: the generation loop skips reproduction
//! on its final iteration, and reproduction is the only step that draws RNG
//! after the archive insert. So a run of length `k` reaches byte-identical
//! archive and history state to the first `k` generations of a run of length
//! `N`. The harness does not assume this — it checks it, and reports
//! `prefix_consistent` so a reader can see the check ran.

use std::{env, process, time::Instant};

use axiom::{
    Archive, EvolutionConfig, EvolutionReport, SearchMode, TaskKind, evolution::GenerationSummary,
    run_evolution,
};
use serde_json::{Value, json};

struct Arguments {
    seeds: Vec<u64>,
    generations: usize,
    population: usize,
    steps: usize,
    archive_width: usize,
    archive_height: usize,
    task: TaskKind,
    trajectory: bool,
}

impl Default for Arguments {
    fn default() -> Self {
        let defaults = EvolutionConfig::default();
        Self {
            seeds: vec![1, 2, 3, 4, 5],
            generations: defaults.generations,
            population: defaults.population_size,
            steps: defaults.evaluation_steps,
            archive_width: defaults.archive_width,
            archive_height: defaults.archive_height,
            task: defaults.task,
            trajectory: true,
        }
    }
}

fn main() {
    let arguments = parse_arguments();
    let started = Instant::now();

    let seeds: Vec<Value> = arguments
        .seeds
        .iter()
        .map(|&seed| measure_seed(&arguments, seed))
        .collect();

    let first_seed = arguments.seeds[0];
    let repeat_a = run(&arguments, first_seed, arguments.generations);
    let repeat_b = run(&arguments, first_seed, arguments.generations);

    let document = json!({
        "schema_version": 1,
        "harness": "axiom-qd-sweep",
        "engine": "axiom::run_evolution (MAP-Elites, real fitness evaluation)",
        "config": {
            "seeds": arguments.seeds,
            "generations": arguments.generations,
            "population": arguments.population,
            "evaluation_steps": arguments.steps,
            "task": task_name(arguments.task),
            "search_mode": "map-elites",
            "archive_width": arguments.archive_width,
            "archive_height": arguments.archive_height,
            "archive_cells": arguments.archive_width * arguments.archive_height,
        },
        "behaviour_space": {
            "x_axis": repeat_a.archive.axes().0,
            "y_axis": repeat_a.archive.axes().1,
        },
        "fitness": {
            "definition": "axiom::fitness::evaluate",
            "note": "fitness can be negative: the tumble penalty is subtracted \
                     without a floor, so filling a new cell with a tumbling \
                     creature can lower the raw QD-score.",
        },
        "determinism": {
            "method": "same seed and config, two in-process runs, full EvolutionReport compared",
            "seed": first_seed,
            "reports_identical": repeat_a == repeat_b,
            "report_digest_a": digest(&repeat_a),
            "report_digest_b": digest(&repeat_b),
            "digest_algorithm": "fnv1a64-over-serde-json",
        },
        "seeds": seeds,
        "wall_clock_seconds": started.elapsed().as_secs_f64(),
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&document).expect("harness report should serialize")
    );
}

fn measure_seed(arguments: &Arguments, seed: u64) -> Value {
    let started = Instant::now();
    let full = run(arguments, seed, arguments.generations);

    let (trajectory, prefix_consistent) = if arguments.trajectory {
        let mut points = Vec::with_capacity(arguments.generations);
        let mut consistent = true;
        for length in 1..=arguments.generations {
            let prefix = run(arguments, seed, length);
            consistent &= prefix.history.as_slice() == &full.history[..length];
            points.push(trajectory_point(
                prefix.history.last().expect("history is recorded"),
                &prefix.archive,
            ));
        }
        (Value::Array(points), Some(consistent))
    } else {
        (Value::Null, None)
    };

    let fitness: Vec<f32> = full
        .archive
        .elites()
        .map(|elite| elite.evaluation.fitness)
        .collect();
    let elites: Vec<Value> = full
        .archive
        .elites()
        .map(|elite| {
            json!({
                "cell": [elite.cell.0, elite.cell.1],
                "fitness": elite.evaluation.fitness,
                "stable_distance": elite.evaluation.metrics.stable_distance,
                "body_count": elite.evaluation.metrics.body_count,
                "generation": elite.generation,
            })
        })
        .collect();

    json!({
        "seed": seed,
        "trajectory": trajectory,
        "prefix_consistent": prefix_consistent,
        "final": {
            "generations": full.generations,
            "evaluated_genomes": full.evaluated_count,
            "occupied_cells": full.archive.occupied_count(),
            "coverage": full.archive.coverage(),
            "qd_score": qd_score(&full.archive),
            "qd_score_positive": qd_score_positive(&full.archive),
            "negative_fitness_elites": fitness.iter().filter(|value| **value < 0.0).count(),
            "best_fitness": full.best_evaluation.fitness,
            "best_controller": full.best_genome.controller.as_str(),
            "best_stable_distance": full.best_evaluation.metrics.stable_distance,
            "elites": elites,
            "report_digest": digest(&full),
        },
        "seconds": started.elapsed().as_secs_f64(),
    })
}

fn trajectory_point(summary: &GenerationSummary, archive: &Archive) -> Value {
    json!({
        "generation": summary.generation,
        "occupied_cells": summary.occupied_cells,
        "coverage": summary.archive_coverage,
        "qd_score": qd_score(archive),
        "qd_score_positive": qd_score_positive(archive),
        "best_fitness": summary.best_fitness,
        "mean_fitness": summary.mean_fitness,
    })
}

fn run(arguments: &Arguments, seed: u64, generations: usize) -> EvolutionReport {
    run_evolution(EvolutionConfig {
        seed,
        population_size: arguments.population,
        generations,
        evaluation_steps: arguments.steps,
        task: arguments.task,
        controller: None,
        search_mode: SearchMode::MapElites,
        archive_width: arguments.archive_width,
        archive_height: arguments.archive_height,
    })
    .unwrap_or_else(|error| fail(&format!("evolution config rejected: {error}")))
}

/// Sum of elite fitness over occupied cells — the standard QD-score.
fn qd_score(archive: &Archive) -> f64 {
    archive
        .elites()
        .map(|elite| f64::from(elite.evaluation.fitness))
        .sum()
}

/// QD-score with each cell floored at zero.
///
/// The literature assumes non-negative fitness; Axiom's does not have a floor,
/// so both are reported and the notebook says which one it is reading.
fn qd_score_positive(archive: &Archive) -> f64 {
    archive
        .elites()
        .map(|elite| f64::from(elite.evaluation.fitness).max(0.0))
        .sum()
}

fn digest(report: &EvolutionReport) -> String {
    let serialized = serde_json::to_vec(report).expect("evolution report should serialize");
    let hash = serialized
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("{hash:016x}")
}

fn task_name(task: TaskKind) -> &'static str {
    match task {
        TaskKind::FlatRun => "flat-run",
        TaskKind::RoughTerrain => "rough-terrain",
        TaskKind::Recovery => "recovery",
    }
}

fn parse_arguments() -> Arguments {
    let mut arguments = Arguments::default();
    let raw: Vec<String> = env::args().skip(1).collect();
    let mut index = 0;

    while index < raw.len() {
        match raw[index].as_str() {
            "--seeds" => {
                arguments.seeds = next(&raw, &mut index, "seeds")
                    .split(',')
                    .map(|seed| {
                        seed.trim()
                            .parse()
                            .unwrap_or_else(|_| fail(&format!("invalid seed: {seed}")))
                    })
                    .collect();
                if arguments.seeds.is_empty() {
                    fail("at least one seed is required");
                }
            }
            "--generations" => arguments.generations = parse_next(&raw, &mut index, "generations"),
            "--population" => arguments.population = parse_next(&raw, &mut index, "population"),
            "--steps" => arguments.steps = parse_next(&raw, &mut index, "steps"),
            "--archive-width" => {
                arguments.archive_width = parse_next(&raw, &mut index, "archive-width");
            }
            "--archive-height" => {
                arguments.archive_height = parse_next(&raw, &mut index, "archive-height");
            }
            "--task" => {
                let value = next(&raw, &mut index, "task");
                arguments.task = match value.as_str() {
                    "flat" | "flat-run" => TaskKind::FlatRun,
                    "rough" | "rough-terrain" => TaskKind::RoughTerrain,
                    "recovery" => TaskKind::Recovery,
                    other => fail(&format!("unknown task: {other}")),
                };
            }
            "--no-trajectory" => arguments.trajectory = false,
            other => fail(&format!("unknown option: {other}")),
        }
        index += 1;
    }

    if arguments.generations == 0 {
        fail("generations must be greater than zero");
    }
    arguments
}

fn next(raw: &[String], index: &mut usize, label: &str) -> String {
    *index += 1;
    raw.get(*index)
        .cloned()
        .unwrap_or_else(|| fail(&format!("missing value for {label}")))
}

fn parse_next<T: std::str::FromStr>(raw: &[String], index: &mut usize, label: &str) -> T {
    next(raw, index, label)
        .parse()
        .unwrap_or_else(|_| fail(&format!("invalid value for {label}")))
}

fn fail(message: &str) -> ! {
    eprintln!("qd_sweep: {message}");
    process::exit(2);
}
