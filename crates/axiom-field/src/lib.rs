//! Axiom Field Lab: a rigid-body simulator for one fixed, buildable robot.
//!
//! Axiom's own sandbox abstracts locomotion: a creature is a point mass and an
//! action contributes directly to a thrust term, so a constant saturated
//! output is close to optimal and no gait is required. That is a fine search
//! toy and a useless model of hardware. This crate exists to be the honest
//! counterpart — real links, real joints, real contact — so that a controller
//! evolved here has some claim to meaning on a physical robot.
//!
//! Everything physical lives in `robot.toml`. Calibrating the model against a
//! real machine is meant to be a diff of that file, never a code change.

pub mod config;
pub mod controller;
pub mod evaluate;
pub mod gait;
pub mod perturb;
pub mod search;
pub mod servo;
pub mod sim;
pub mod trajectory;

pub use config::RobotConfig;
pub use controller::CpgGenome;
pub use evaluate::{EnsembleScore, Outcome, evaluate, evaluate_ensemble};
pub use gait::{Gait, sine_gait};
pub use perturb::{Perturbation, Spread};
pub use search::{FieldElite, HoldoutReport, SearchConfig, SearchReport, holdout, run_search};
pub use servo::Servo;
pub use sim::{ACTUATOR_COUNT, FieldSim, LEG_COUNT, Sample};
pub use trajectory::{Trajectory, TrajectorySummary};
