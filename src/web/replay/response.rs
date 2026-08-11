//! Replay response assembly: the serialized shapes the browser consumes.
//!
//! These structs are the wire contract for `/api/replay`; the 3D journal and
//! the 2D viewer both decode them, so field names are not free to churn.

use std::collections::HashSet;

use serde::Serialize;

use crate::{
    EvolutionConfig, Genome, SearchMode, TaskKind, animation::capture_replay, evaluate, rng::Rng,
    run_evolution,
};

use super::request::{ReplayMode, ReplayRequest, ReplayRequestError};

#[derive(Serialize)]
struct ReplayErrorResponse {
    error: &'static str,
    cost: usize,
    max_cost: usize,
}

#[derive(Serialize)]
struct ReplayResponse {
    genome_id: Option<u64>,
    controller: &'static str,
    task: &'static str,
    seed: u64,
    source: &'static str,
    generations: usize,
    population: usize,
    evaluation_steps: usize,
    fitness: f32,
    best_distance: f32,
    stable_distance: f32,
    uprightness: f32,
    stability: f32,
    terminal_tilt: f32,
    evolution_history: Vec<ReplayGenerationResponse>,
    lineage: Vec<ReplayLineageResponse>,
    archive: Option<ReplayArchiveResponse>,
    dt: f32,
    body: Vec<ReplayBodyNode>,
    frames: Vec<ReplayFrameResponse>,
}

#[derive(Serialize)]
struct ReplayArchiveResponse {
    x_axis: &'static str,
    y_axis: &'static str,
    width: usize,
    height: usize,
    selected_cell: [usize; 2],
    cells: Vec<ReplayArchiveCellResponse>,
}

#[derive(Serialize)]
struct ReplayArchiveCellResponse {
    cell: [usize; 2],
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    mutation_summary: String,
    fitness: f32,
    stable_distance: f32,
    body_count: f32,
    actuator_count: f32,
}

#[derive(Serialize)]
struct ReplayGenerationResponse {
    generation: usize,
    best_fitness: f32,
    mean_fitness: f32,
    archive_coverage: f32,
    occupied_cells: usize,
}

#[derive(Serialize)]
struct ReplayLineageResponse {
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    controller: &'static str,
    body_count: usize,
    actuator_count: usize,
    mutation_summary: String,
}

#[derive(Serialize)]
struct ReplayBodyNode {
    id: usize,
    parent: Option<usize>,
    size: [f32; 2],
    actuator: f32,
}

#[derive(Serialize)]
struct ReplayFrameResponse {
    time: f32,
    root: [f32; 2],
    tilt: f32,
    bodies: Vec<[f32; 2]>,
    joints: Vec<[[f32; 2]; 2]>,
}

pub(super) fn replay_error_json(error: ReplayRequestError) -> String {
    match error {
        ReplayRequestError::EvolvedBudgetExceeded { cost, max_cost } => {
            let response = ReplayErrorResponse {
                error: "evolved replay request exceeds the interactive preview budget",
                cost,
                max_cost,
            };
            serde_json::to_string(&response).expect("serializing replay error cannot fail")
        }
    }
}

pub(crate) fn replay_json(request: ReplayRequest) -> String {
    let (
        genome,
        evaluation,
        genome_id,
        generations,
        population_size,
        evolution_history,
        lineage,
        archive,
    ) = match request.mode {
        ReplayMode::Minimal => {
            let mut rng = Rng::new(request.seed);
            let genome = Genome::minimal(request.controller, &mut rng);
            let evaluation = evaluate(&genome, request.task, request.evaluation_steps);
            (genome, evaluation, None, 0, 1, Vec::new(), Vec::new(), None)
        }
        ReplayMode::Evolved => {
            let report = run_evolution(EvolutionConfig {
                seed: request.seed,
                population_size: request.population_size,
                generations: request.generations,
                evaluation_steps: request.evaluation_steps,
                task: request.task,
                controller: Some(request.controller),
                search_mode: SearchMode::MapElites,
                ..EvolutionConfig::default()
            })
            .expect("validated replay evolution config should be valid");
            let selected = request
                .archive_cell()
                .and_then(|cell| report.archive.elite_at(cell))
                .or_else(|| report.archive.best())
                .expect("a non-empty evolution run should populate the archive");
            let genome = selected.genome.clone();
            let evaluation = selected.evaluation.clone();
            let genome_id = selected.genome_id;
            let lineage = replay_lineage_response(&report.lineage, genome_id);
            let archive = Some(replay_archive_response(&report.archive, selected.cell));
            let history = report.history;
            (
                genome,
                evaluation,
                Some(genome_id),
                request.generations,
                request.population_size,
                history,
                lineage,
                archive,
            )
        }
    };
    let frames = capture_replay(&genome, request.task, request.frames, request.dt);
    let response = ReplayResponse {
        genome_id,
        controller: genome.controller.as_str(),
        task: task_name(request.task),
        seed: request.seed,
        source: request.mode.as_str(),
        generations,
        population: population_size,
        evaluation_steps: request.evaluation_steps,
        fitness: evaluation.fitness,
        best_distance: evaluation.metrics.distance,
        stable_distance: evaluation.metrics.stable_distance,
        uprightness: evaluation.metrics.uprightness,
        stability: evaluation.metrics.stability,
        terminal_tilt: evaluation.metrics.terminal_tilt,
        evolution_history: evolution_history
            .into_iter()
            .map(|generation| ReplayGenerationResponse {
                generation: generation.generation,
                best_fitness: generation.best_fitness,
                mean_fitness: generation.mean_fitness,
                archive_coverage: generation.archive_coverage,
                occupied_cells: generation.occupied_cells,
            })
            .collect(),
        lineage,
        archive,
        dt: request.dt,
        body: genome
            .body
            .nodes
            .iter()
            .map(|node| ReplayBodyNode {
                id: node.id,
                parent: node.parent,
                size: [node.size.x, node.size.y],
                actuator: node.actuator_strength,
            })
            .collect(),
        frames: frames
            .iter()
            .map(|frame| ReplayFrameResponse {
                time: frame.time,
                root: [frame.root_position.x, frame.root_position.y],
                tilt: frame.tilt,
                bodies: frame
                    .body_centers
                    .iter()
                    .map(|center| [center.x, center.y])
                    .collect(),
                joints: frame
                    .joint_segments
                    .iter()
                    .map(|(start, end)| [[start.x, start.y], [end.x, end.y]])
                    .collect(),
            })
            .collect(),
    };

    serde_json::to_string(&response).expect("serializing replay response cannot fail")
}

fn replay_archive_response(
    archive: &crate::qd::Archive,
    selected_cell: (usize, usize),
) -> ReplayArchiveResponse {
    ReplayArchiveResponse {
        x_axis: archive.x_axis.label(),
        y_axis: archive.y_axis.label(),
        width: archive.width(),
        height: archive.height(),
        selected_cell: [selected_cell.0, selected_cell.1],
        cells: archive
            .elites()
            .map(|elite| ReplayArchiveCellResponse {
                cell: [elite.cell.0, elite.cell.1],
                genome_id: elite.genome_id,
                parent_id: elite.parent_id,
                generation: elite.generation,
                mutation_summary: elite.mutation_summary.clone(),
                fitness: elite.evaluation.fitness,
                stable_distance: elite.evaluation.metrics.stable_distance,
                body_count: elite.evaluation.metrics.body_count,
                actuator_count: elite.evaluation.metrics.actuator_count,
            })
            .collect(),
    }
}

fn replay_lineage_response(
    lineage: &[crate::evolution::LineageRecord],
    selected_genome_id: u64,
) -> Vec<ReplayLineageResponse> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current_id = Some(selected_genome_id);

    while let Some(genome_id) = current_id {
        if !seen.insert(genome_id) {
            break;
        }
        let Some(record) = lineage.iter().find(|record| record.genome_id == genome_id) else {
            break;
        };
        chain.push(ReplayLineageResponse {
            genome_id: record.genome_id,
            parent_id: record.parent_id,
            generation: record.generation,
            controller: record.controller.as_str(),
            body_count: record.body_count,
            actuator_count: record.actuator_count,
            mutation_summary: record.mutation_summary.clone(),
        });
        current_id = record.parent_id;
    }

    chain.reverse();
    chain
}

fn task_name(task: TaskKind) -> &'static str {
    match task {
        TaskKind::FlatRun => "flat",
        TaskKind::RoughTerrain => "rough",
        TaskKind::Recovery => "recovery",
    }
}

#[cfg(test)]
mod tests {
    use super::super::request::{ReplayMode, ReplayRequest};
    use super::replay_json;

    #[test]
    fn replay_json_contains_requested_frame_count_plus_initial_pose() {
        let json = replay_json(ReplayRequest {
            frames: 2,
            ..ReplayRequest::default()
        });
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("replay should serialize as JSON");
        let frames = value["frames"]
            .as_array()
            .expect("frames should be an array");

        assert!(value.get("body").is_some());
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn evolved_replay_honors_requested_controller() {
        let json = replay_json(ReplayRequest {
            mode: ReplayMode::Evolved,
            controller: crate::policy::ControllerKind::Recurrent,
            population_size: 4,
            generations: 1,
            evaluation_steps: 20,
            frames: 1,
            ..ReplayRequest::default()
        });
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("replay should serialize as JSON");

        assert_eq!(value["source"], "evolved");
        assert_eq!(value["controller"], "recurrent");
        assert_eq!(value["evolution_history"].as_array().map(Vec::len), Some(1));
        assert!(value["evolution_history"][0]["best_fitness"].is_number());
        assert_eq!(value["archive"]["width"], 12);
        assert_eq!(value["archive"]["height"], 8);
        assert!(
            value["archive"]["cells"]
                .as_array()
                .is_some_and(|cells| !cells.is_empty())
        );
        assert!(value["archive"]["selected_cell"].is_array());
    }

    #[test]
    fn evolved_replay_exposes_truthful_selected_lineage() {
        let json = replay_json(ReplayRequest {
            mode: ReplayMode::Evolved,
            seed: 71,
            generations: 3,
            population_size: 8,
            evaluation_steps: 20,
            frames: 1,
            ..ReplayRequest::default()
        });
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("replay should serialize as JSON");
        let genome_id = value["genome_id"]
            .as_u64()
            .expect("evolved replay should identify the selected genome");
        let lineage = value["lineage"]
            .as_array()
            .expect("evolved replay should include a lineage chain");

        assert!(!lineage.is_empty());
        assert_eq!(
            lineage.last().unwrap()["genome_id"].as_u64(),
            Some(genome_id)
        );
        assert!(lineage.last().unwrap()["mutation_summary"].is_string());
        assert!(
            value["archive"]["cells"]
                .as_array()
                .unwrap()
                .iter()
                .all(|cell| cell["genome_id"].is_u64() && cell["generation"].is_u64())
        );
    }

    #[test]
    fn evolved_replay_can_select_an_occupied_archive_cell() {
        let initial = replay_json(ReplayRequest {
            mode: ReplayMode::Evolved,
            population_size: 4,
            generations: 1,
            evaluation_steps: 20,
            frames: 1,
            ..ReplayRequest::default()
        });
        let initial: serde_json::Value = serde_json::from_str(&initial).unwrap();
        let cell = initial["archive"]["cells"][0]["cell"]
            .as_array()
            .expect("archive cell should have coordinates");
        let selected = replay_json(ReplayRequest {
            mode: ReplayMode::Evolved,
            population_size: 4,
            generations: 1,
            evaluation_steps: 20,
            frames: 1,
            archive_cell_x: cell[0].as_u64().map(|value| value as usize),
            archive_cell_y: cell[1].as_u64().map(|value| value as usize),
            ..ReplayRequest::default()
        });
        let selected: serde_json::Value = serde_json::from_str(&selected).unwrap();

        assert_eq!(
            selected["archive"]["selected_cell"],
            initial["archive"]["cells"][0]["cell"]
        );
        assert_eq!(
            selected["fitness"],
            initial["archive"]["cells"][0]["fitness"]
        );
    }
}
