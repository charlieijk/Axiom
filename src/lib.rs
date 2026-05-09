pub mod animation;
pub mod checkpoint;
pub mod evolution;
pub mod fitness;
pub mod genome;
pub mod math;
pub mod network;
pub mod policy;
pub mod qd;
pub mod rng;
pub mod simulation;
pub mod web;

pub use checkpoint::{CHECKPOINT_VERSION, EvolutionCheckpoint, load_checkpoint, save_checkpoint};
pub use evolution::{
    EvolutionConfig, EvolutionConfigError, EvolutionReport, GenerationSummary, LineageRecord,
    SearchMode, run_evolution, run_evolution_with_progress,
};
pub use fitness::{Evaluation, TaskKind, evaluate};
pub use genome::{BodyGenome, Genome};
pub use policy::{Brain, BrainState, ControllerKind};
pub use qd::{Archive, Axis};
