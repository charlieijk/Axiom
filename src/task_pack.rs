use serde::{Deserialize, Serialize};

use crate::{
    fitness::{Evaluation, Metrics, TaskKind, evaluate},
    genome::Genome,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TaskPackKind {
    RoughInspection,
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
                TaskScenario::new("rough traverse", TaskKind::RoughTerrain, default_steps, 0.5),
                TaskScenario::new(
                    "tilt recovery",
                    TaskKind::Recovery,
                    (default_steps * 3 / 4).max(20),
                    0.25,
                ),
                TaskScenario::new("step field", TaskKind::StepField, default_steps, 0.25),
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskScenario {
    pub name: String,
    pub task: TaskKind,
    pub steps: usize,
    pub weight: f32,
}

impl TaskScenario {
    pub fn new(name: impl Into<String>, task: TaskKind, steps: usize, weight: f32) -> Self {
        Self {
            name: name.into(),
            task,
            steps,
            weight,
        }
    }

    pub fn single_task(task: TaskKind, steps: usize) -> Self {
        Self::new(task.as_str(), task, steps, 1.0)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScenarioEvaluation {
    pub scenario: TaskScenario,
    pub evaluation: Evaluation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
            let evaluation = evaluate(genome, scenario.task, scenario.steps);
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
    fn pack_evaluation_is_deterministic_for_fixed_genome() {
        let mut rng = Rng::new(64);
        let genome = Genome::minimal(ControllerKind::Cpg, &mut rng);

        let first = evaluate_pack(&genome, TaskPackKind::RoughInspection, 40);
        let second = evaluate_pack(&genome, TaskPackKind::RoughInspection, 40);

        assert_eq!(first.scenarios.len(), 3);
        assert_eq!(first.steps, second.steps);
        assert!((first.fitness - second.fitness).abs() < 0.0001);
        assert!((first.metrics.distance - second.metrics.distance).abs() < 0.0001);
    }
}
