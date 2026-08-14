use crate::genome::{BodyGenome, Genome, sensor_count};
use crate::policy::Brain;
use crate::simulation::{Simulation, Snapshot, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TaskKind {
    FlatRun,
    RoughTerrain,
    Recovery,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Metrics {
    pub distance: f32,
    pub stable_distance: f32,
    pub jump_height: f32,
    pub uprightness: f32,
    pub stability: f32,
    pub terminal_tilt: f32,
    pub body_count: f32,
    pub actuator_count: f32,
    pub energy: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Evaluation {
    pub fitness: f32,
    pub metrics: Metrics,
    pub steps: usize,
}

impl TaskKind {
    pub fn world(self) -> World {
        match self {
            Self::FlatRun => World::flat(),
            Self::RoughTerrain => World::rough(),
            Self::Recovery => {
                let mut world = World::rough();
                world.friction = 0.28;
                world
            }
        }
    }
}

pub fn evaluate(genome: &Genome, task: TaskKind, steps: usize) -> Evaluation {
    let mut simulation = Simulation::new(task.world());
    simulation.spawn_creature(&genome.body);
    let brain = Brain::from_genome(genome);
    let mut brain_state = brain.reset_state();
    let mut brain_workspace = brain.workspace();
    let mut snapshot = simulation.snapshot();
    let mut observations = Vec::with_capacity(sensor_count(&genome.body));
    let mut max_height = 0.0_f32;
    let mut upright_accumulator = 0.0_f32;
    let mut angular_control_accumulator = 0.0_f32;

    for _ in 0..steps {
        observation_vector_into(&snapshot, &genome.body, &mut observations);
        let actions = brain.think_into(&observations, &mut brain_state, &mut brain_workspace);
        simulation.step(genome, actions, 0.05);

        simulation.snapshot_into(&mut snapshot);
        max_height = max_height.max(snapshot.root_position.y - snapshot.terrain_height);
        upright_accumulator += tilt_stability(snapshot.tilt);
        angular_control_accumulator +=
            (1.0 - snapshot.angular_velocity.abs() / 3.0).clamp(0.0, 1.0);
    }

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

    let terrain_multiplier = match task {
        TaskKind::FlatRun => 1.0,
        TaskKind::RoughTerrain => 1.25,
        TaskKind::Recovery => 1.15,
    };
    let controlled_distance = stable_distance * 11.0 * terrain_multiplier;
    let raw_progress = distance * 1.4 * terrain_multiplier;
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

pub fn observation_vector(snapshot: &Snapshot, body: &BodyGenome) -> Vec<f32> {
    let mut observations = Vec::with_capacity(sensor_count(body));
    observation_vector_into(snapshot, body, &mut observations);
    observations
}

pub(crate) fn observation_vector_into(
    snapshot: &Snapshot,
    body: &BodyGenome,
    observations: &mut Vec<f32>,
) {
    observations.clear();
    observations.extend_from_slice(&[
        snapshot.time,
        snapshot.root_position.x,
        snapshot.root_position.y,
        snapshot.root_velocity.x,
        snapshot.root_velocity.y,
        snapshot.tilt,
        snapshot.angular_velocity,
        snapshot.energy_spent,
        snapshot.terrain_height,
        snapshot.terrain_slope,
        body.body_count() as f32 / 10.0,
        body.actuator_count() as f32 / 10.0,
    ]);

    for joint in snapshot.joints.iter().filter(|joint| {
        body.nodes
            .get(joint.node_id)
            .map(|node| node.is_actuated())
            .unwrap_or(false)
    }) {
        observations.push(joint.angle);
        observations.push(joint.angular_velocity);
    }

    for node in &body.nodes {
        observations.push(node.size.x);
        observations.push(node.size.y);
    }

    observations.resize(sensor_count(body), 0.0);
}

#[cfg(test)]
mod tests {
    use crate::{
        Genome,
        math::{Attachment, Vec2},
        policy::ControllerKind,
        rng::Rng,
        simulation::{JointAnchor, JointState, Simulation, Snapshot},
    };

    use super::{TaskKind, observation_vector, observation_vector_into};

    #[test]
    fn observation_vector_matches_genome_sensor_count() {
        let mut rng = Rng::new(10);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let mut simulation = Simulation::new(TaskKind::RoughTerrain.world());
        simulation.spawn_creature(&genome.body);

        let observations = observation_vector(&simulation.snapshot(), &genome.body);

        assert_eq!(observations.len(), genome.brain.input_count);
    }

    #[test]
    fn observation_vector_uses_actuated_joint_slots() {
        let mut body = crate::BodyGenome::seed_quadruped();
        body.nodes[1].actuator_strength = 0.0;
        let snapshot = Snapshot {
            time: 0.0,
            root_position: Vec2::ZERO,
            root_velocity: Vec2::ZERO,
            tilt: 0.0,
            angular_velocity: 0.0,
            energy_spent: 0.0,
            terrain_height: 0.0,
            terrain_slope: 0.0,
            joints: vec![
                test_joint(1, Attachment::Left, 10.0),
                test_joint(2, Attachment::Right, 20.0),
            ],
        };

        let observations = observation_vector(&snapshot, &body);

        assert_eq!(observations.len(), crate::genome::sensor_count(&body));
        assert_eq!(observations[12], 20.0);
    }

    #[test]
    fn reusable_observation_buffer_matches_owned_observations() {
        let mut rng = Rng::new(12);
        let genome = Genome::minimal(ControllerKind::Cpg, &mut rng);
        let mut simulation = Simulation::new(TaskKind::RoughTerrain.world());
        simulation.spawn_creature(&genome.body);
        simulation.step(&genome, &[0.25, -0.5, 0.75, -1.0], 0.05);
        let snapshot = simulation.snapshot();
        let expected = observation_vector(&snapshot, &genome.body);
        let mut actual = vec![99.0; expected.len() + 8];

        observation_vector_into(&snapshot, &genome.body, &mut actual);

        assert_eq!(actual, expected);
    }

    fn test_joint(node_id: usize, attachment: Attachment, angle: f32) -> JointState {
        JointState {
            node_id,
            attachment,
            anchor: JointAnchor {
                parent_anchor: Vec2::ZERO,
                child_anchor: Vec2::ZERO,
            },
            angle,
            angular_velocity: 0.0,
        }
    }
}
