//! Replay request parsing, defaults, and budget validation.
//!
//! Parsing is deliberately lenient: an unparseable value keeps its default and
//! an out-of-range value clamps, so a malformed query never errors. The budget
//! check is the only thing that can reject a request.

use crate::{TaskKind, policy::ControllerKind};

// `/api/replay?mode=evolved` is an interactive preview, not a full experiment runner.
// Defaults stay around 60k simulated evaluation steps; longer searches belong in the CLI.
pub(super) const MAX_EVOLVED_REPLAY_COST: usize = 250_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReplayRequest {
    pub(crate) mode: ReplayMode,
    pub(crate) controller: ControllerKind,
    pub(crate) task: TaskKind,
    pub(crate) seed: u64,
    pub(crate) frames: usize,
    pub(crate) dt: f32,
    pub(crate) generations: usize,
    pub(crate) population_size: usize,
    pub(crate) evaluation_steps: usize,
    pub(crate) archive_cell_x: Option<usize>,
    pub(crate) archive_cell_y: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplayMode {
    Minimal,
    Evolved,
}

impl ReplayMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Evolved => "evolved",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ReplayRequestError {
    EvolvedBudgetExceeded { cost: usize, max_cost: usize },
}

impl Default for ReplayRequest {
    fn default() -> Self {
        Self {
            mode: ReplayMode::Minimal,
            controller: ControllerKind::Cpg,
            task: TaskKind::RoughTerrain,
            seed: 19,
            frames: 220,
            dt: 0.05,
            generations: 12,
            population_size: 28,
            evaluation_steps: 180,
            archive_cell_x: None,
            archive_cell_y: None,
        }
    }
}

impl ReplayRequest {
    pub(super) fn archive_cell(self) -> Option<(usize, usize)> {
        self.archive_cell_x.zip(self.archive_cell_y)
    }
}

pub(crate) fn parse_replay_request(query: &str) -> ReplayRequest {
    let mut request = ReplayRequest::default();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = url_decode(value);
        match key {
            "mode" | "source" => {
                if let Some(mode) = parse_replay_mode(&value) {
                    request.mode = mode;
                }
            }
            "controller" => {
                if let Some(controller) = parse_controller(&value) {
                    request.controller = controller;
                }
            }
            "task" => {
                if let Some(task) = parse_task(&value) {
                    request.task = task;
                }
            }
            "seed" => {
                if let Ok(seed) = value.parse() {
                    request.seed = seed;
                }
            }
            "frames" => {
                if let Ok(frames) = value.parse::<usize>() {
                    request.frames = frames.clamp(1, 900);
                }
            }
            "generations" => {
                if let Ok(generations) = value.parse::<usize>() {
                    request.generations = generations.clamp(1, 40);
                }
            }
            "population" | "population_size" => {
                if let Ok(population_size) = value.parse::<usize>() {
                    request.population_size = population_size.clamp(4, 96);
                }
            }
            "evaluation_steps" | "evolve_steps" => {
                if let Ok(evaluation_steps) = value.parse::<usize>() {
                    request.evaluation_steps = evaluation_steps.clamp(20, 500);
                }
            }
            "cell_x" | "archive_x" => {
                request.archive_cell_x = value.parse().ok();
            }
            "cell_y" | "archive_y" => {
                request.archive_cell_y = value.parse().ok();
            }
            _ => {}
        }
    }

    request
}

pub(super) fn validate_replay_request(request: &ReplayRequest) -> Result<(), ReplayRequestError> {
    if request.mode == ReplayMode::Evolved {
        let cost = evolved_replay_cost(request);
        if cost > MAX_EVOLVED_REPLAY_COST {
            return Err(ReplayRequestError::EvolvedBudgetExceeded {
                cost,
                max_cost: MAX_EVOLVED_REPLAY_COST,
            });
        }
    }

    Ok(())
}

fn evolved_replay_cost(request: &ReplayRequest) -> usize {
    request
        .generations
        .saturating_mul(request.population_size)
        .saturating_mul(request.evaluation_steps)
}

fn parse_replay_mode(value: &str) -> Option<ReplayMode> {
    match value {
        "minimal" | "seed" | "raw" => Some(ReplayMode::Minimal),
        "evolved" | "evolve" | "champion" => Some(ReplayMode::Evolved),
        _ => None,
    }
}

fn parse_controller(value: &str) -> Option<ControllerKind> {
    match value {
        "feedforward" | "ff" => Some(ControllerKind::FeedForward),
        "recurrent" | "rnn" => Some(ControllerKind::Recurrent),
        "cpg" => Some(ControllerKind::Cpg),
        _ => None,
    }
}

fn parse_task(value: &str) -> Option<TaskKind> {
    match value {
        "flat" | "flat-run" => Some(TaskKind::FlatRun),
        "rough" | "rough-terrain" => Some(TaskKind::RoughTerrain),
        "recovery" => Some(TaskKind::Recovery),
        _ => None,
    }
}

fn url_decode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        match byte {
            b'+' => output.push(' '),
            b'%' => {
                let first = chars.next();
                let second = chars.next();
                let decoded = first.zip(second).and_then(|(first, second)| {
                    let encoded = [first, second];
                    std::str::from_utf8(&encoded)
                        .ok()
                        .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                });
                if let Some(decoded) = decoded {
                    output.push(decoded as char);
                }
            }
            _ => output.push(byte as char),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{ReplayMode, ReplayRequest, parse_replay_request, validate_replay_request};

    #[test]
    fn evolved_replay_default_stays_within_interactive_budget() {
        let request = ReplayRequest {
            mode: ReplayMode::Evolved,
            ..ReplayRequest::default()
        };

        assert!(validate_replay_request(&request).is_ok());
    }

    #[test]
    fn malformed_query_values_fall_back_to_interactive_defaults() {
        // Parsing is deliberately lenient: unparseable values keep their
        // defaults, out-of-range values clamp, and unknown keys are ignored,
        // so a malformed query never produces an error — only the evolved
        // budget check (below) can reject a request.
        let defaults = ReplayRequest::default();
        let request = parse_replay_request(
            "mode=warp&seed=abc&frames=&generations=minus-three&population=lots&evaluation_steps=9999&cell_x=west&unknown=1",
        );

        assert_eq!(request.mode, defaults.mode);
        assert_eq!(request.seed, defaults.seed);
        assert_eq!(request.frames, defaults.frames);
        assert_eq!(request.generations, defaults.generations);
        assert_eq!(request.population_size, defaults.population_size);
        assert_eq!(request.evaluation_steps, 500, "9999 parses, then clamps");
        assert_eq!(request.archive_cell_x, None);
        assert!(validate_replay_request(&request).is_ok());
    }
}
