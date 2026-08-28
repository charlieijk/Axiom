pub mod animation;
pub mod checkpoint;
pub mod evolution;
pub mod fitness;
pub mod genome;
pub mod handoff;
pub mod math;
pub mod network;
pub mod policy;
pub mod qd;
pub mod report;
pub mod rng;
pub mod simulation;
pub mod task_pack;
pub mod web;

pub use checkpoint::{EvolutionCheckpoint, load_checkpoint, save_checkpoint};
pub use evolution::{
    EvolutionConfig, EvolutionConfigError, EvolutionReport, LineageRecord, SearchMode,
    run_evolution, run_evolution_with_pack,
};
pub use fitness::{Evaluation, TaskKind, evaluate};
pub use genome::{BodyGenome, Genome};
pub use handoff::{CandidateShortlist, save_candidate_shortlist};
pub use policy::{Brain, BrainState, ControllerKind};
pub use qd::{Archive, Axis, AxisSpec, Grid};
pub use report::{render_report_markdown, save_report_json, save_report_markdown};
pub use task_pack::{PackEvaluation, ScenarioTask, TaskPackKind, TaskScenario, evaluate_pack};
