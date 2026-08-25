use serde::{Deserialize, Serialize};

use crate::{
    fitness::{Evaluation, Metrics, TaskKind, evaluate, observation_vector},
    genome::Genome,
    policy::Brain,
    simulation::{Simulation, TerrainKind, World},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TaskPackKind {
    RoughInspection,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ScenarioTask {
    FlatRun,
    RoughTerrain,
    StepField,
    Recovery,
}

impl ScenarioTask {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlatRun => "flat",
            Self::RoughTerrain => "rough",
            Self::StepField => "steps",
            Self::Recovery => "recovery",
        }
    }

    fn evaluate(self, genome: &Genome, steps: usize) -> Evaluation {
        match self {
            Self::FlatRun => evaluate(genome, TaskKind::FlatRun, steps),
            Self::RoughTerrain => evaluate(genome, TaskKind::RoughTerrain, steps),
            Self::StepField => evaluate_step_field(genome, steps),
            Self::Recovery => evaluate(genome, TaskKind::Recovery, steps),
        }
    }
}

impl From<TaskKind> for ScenarioTask {
    fn from(task: TaskKind) -> Self {
        match task {
            TaskKind::FlatRun => Self::FlatRun,
            TaskKind::RoughTerrain => Self::RoughTerrain,
            TaskKind::Recovery => Self::Recovery,
        }
    }
}

impl TaskPackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoughInspection => "rough-inspection",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RoughInspection => "Rough inspection",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rough-inspection" | "rough_inspection" | "inspection" => Some(Self::RoughInspection),
            _ => None,
        }
    }

    pub fn scenarios(self, default_steps: usize) -> Vec<TaskScenario> {
        match self {
            Self::RoughInspection => vec![
                TaskScenario::new(
                    "rough traverse",
                    ScenarioTask::RoughTerrain,
                    default_steps,
                    0.5,
                ),
                TaskScenario::new(
                    "tilt recovery",
                    ScenarioTask::Recovery,
                    (default_steps * 3 / 4).max(20),
                    0.25,
                ),
                TaskScenario::new("step field", ScenarioTask::StepField, default_steps, 0.25),
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskScenario {
    pub name: String,
    pub task: ScenarioTask,
    pub steps: usize,
    pub weight: f32,
}

impl TaskScenario {
    pub fn new(name: impl Into<String>, task: ScenarioTask, steps: usize, weight: f32) -> Self {
        Self {
            name: name.into(),
            task,
            steps,
            weight,
        }
    }

    pub fn single_task(task: TaskKind, steps: usize) -> Self {
        let task = ScenarioTask::from(task);
        Self::new(task.as_str(), task, steps, 1.0)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScenarioEvaluation {
    pub scenario: TaskScenario,
    pub evaluation: Evaluation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PackEvaluation {
    pub pack: TaskPackKind,
    pub fitness: f32,
    pub metrics: Metrics,
    pub steps: usize,
    pub scenarios: Vec<ScenarioEvaluation>,
}

impl PackEvaluation {
    pub fn as_evaluation(&self) -> Evaluation {
        Evaluation {
            fitness: self.fitness,
            metrics: self.metrics.clone(),
            steps: self.steps,
        }
    }
}

pub fn evaluate_pack(genome: &Genome, pack: TaskPackKind, default_steps: usize) -> PackEvaluation {
    let mut weighted_metrics = Metrics::default();
    let mut weighted_fitness = 0.0_f32;
    let mut total_weight = 0.0_f32;
    let mut total_steps = 0_usize;
    let scenarios = pack
        .scenarios(default_steps)
        .into_iter()
        .map(|scenario| {
            let evaluation = scenario.task.evaluate(genome, scenario.steps);
            let weight = scenario.weight.max(0.0);
            total_weight += weight;
            total_steps += evaluation.steps;
            weighted_fitness += evaluation.fitness * weight;
            add_weighted_metrics(&mut weighted_metrics, &evaluation.metrics, weight);

            ScenarioEvaluation {
                scenario,
                evaluation,
            }
        })
        .collect::<Vec<_>>();

    if total_weight > 0.0 {
        weighted_fitness /= total_weight;
        scale_metrics(&mut weighted_metrics, 1.0 / total_weight);
    }

    PackEvaluation {
        pack,
        fitness: weighted_fitness,
        metrics: weighted_metrics,
        steps: total_steps,
        scenarios,
    }
}

fn evaluate_step_field(genome: &Genome, steps: usize) -> Evaluation {
    let mut simulation = Simulation::new(World {
        terrain: TerrainKind::Steps,
        gravity: 9.8,
        friction: 0.2,
    });
    simulation.spawn_creature(&genome.body);
    let brain = Brain::from_genome(genome);
    let mut brain_state = brain.reset_state();
    let mut max_height = 0.0_f32;
    let mut upright_accumulator = 0.0_f32;
    let mut angular_control_accumulator = 0.0_f32;

    for _ in 0..steps {
        let snapshot = simulation.snapshot();
        let observations = observation_vector(&snapshot, &genome.body);
        let actions = brain.think(&observations, &mut brain_state);
        simulation.step(genome, &actions, 0.05);

        let next = simulation.snapshot();
        max_height = max_height.max(next.root_position.y - next.terrain_height);
        upright_accumulator += tilt_stability(next.tilt);
        angular_control_accumulator += (1.0 - next.angular_velocity.abs() / 3.0).clamp(0.0, 1.0);
    }

    let snapshot = simulation.snapshot();
    let distance = snapshot.root_position.x.max(0.0);
    let uprightness = upright_accumulator / steps.max(1) as f32;
    let angular_control = angular_control_accumulator / steps.max(1) as f32;
    let terminal_stability = tilt_stability(snapshot.tilt);
    let stability =
        (uprightness * 0.56 + terminal_stability * 0.32 + angular_control * 0.12).clamp(0.0, 1.0);
    let stable_distance = distance * stability;
    let energy = snapshot.energy_spent;
    let metrics = Metrics {
        distance,
        stable_distance,
        jump_height: max_height,
        uprightness,
        stability,
        terminal_tilt: snapshot.tilt.abs(),
        body_count: genome.body.body_count() as f32,
        actuator_count: genome.body.actuator_count() as f32,
        energy,
    };

    let controlled_distance = stable_distance * 11.0 * 1.2;
    let raw_progress = distance * 1.4 * 1.2;
    let posture_score = uprightness * 8.0 + terminal_stability * 12.0 + stability * 10.0;
    let hop_score = max_height.clamp(0.0, 0.85) * 0.7;
    let tumble_penalty =
        (1.0 - terminal_stability).powi(2) * 18.0 + (1.0 - stability).powi(2) * 8.0;
    let fitness = controlled_distance + raw_progress + posture_score + hop_score
        - tumble_penalty
        - energy * 0.08
        - metrics.body_count * 0.04;

    Evaluation {
        fitness,
        metrics,
        steps,
    }
}

fn tilt_stability(tilt: f32) -> f32 {
    (1.0 - tilt.abs() / 1.5).clamp(0.0, 1.0)
}

fn add_weighted_metrics(total: &mut Metrics, metrics: &Metrics, weight: f32) {
    total.distance += metrics.distance * weight;
    total.stable_distance += metrics.stable_distance * weight;
    total.jump_height += metrics.jump_height * weight;
    total.uprightness += metrics.uprightness * weight;
    total.stability += metrics.stability * weight;
    total.terminal_tilt += metrics.terminal_tilt * weight;
    total.body_count += metrics.body_count * weight;
    total.actuator_count += metrics.actuator_count * weight;
    total.energy += metrics.energy * weight;
}

fn scale_metrics(metrics: &mut Metrics, scale: f32) {
    metrics.distance *= scale;
    metrics.stable_distance *= scale;
    metrics.jump_height *= scale;
    metrics.uprightness *= scale;
    metrics.stability *= scale;
    metrics.terminal_tilt *= scale;
    metrics.body_count *= scale;
    metrics.actuator_count *= scale;
    metrics.energy *= scale;
}

#[cfg(test)]
mod tests {
    use crate::{Genome, policy::ControllerKind, rng::Rng};

    use super::{TaskPackKind, evaluate_pack};

    #[test]
    fn rough_inspection_pack_parses_and_includes_step_field() {
        let pack = TaskPackKind::parse("rough-inspection").expect("pack should parse");
        let scenarios = pack.scenarios(80);

        assert_eq!(pack.as_str(), "rough-inspection");
        assert_eq!(scenarios.len(), 3);
        assert!(
            scenarios
                .iter()
                .any(|scenario| scenario.name == "step field")
        );
        assert!(
            (scenarios
                .iter()
                .map(|scenario| scenario.weight)
                .sum::<f32>()
                - 1.0)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn pack_evaluation_is_deterministic_for_a_fixed_genome() {
        let mut rng = Rng::new(64);
        let genome = Genome::minimal(ControllerKind::Cpg, &mut rng);

        let first = evaluate_pack(&genome, TaskPackKind::RoughInspection, 40);
        let second = evaluate_pack(&genome, TaskPackKind::RoughInspection, 40);

        assert_eq!(first, second);
        assert_eq!(first.scenarios.len(), 3);
        assert!(first.steps > 40);
    }
}
