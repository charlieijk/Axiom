use crate::genome::{BodyGenome, Genome, sensor_count};
use crate::policy::Brain;
use crate::simulation::{Simulation, Snapshot, World};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskKind {
    FlatRun,
    RoughTerrain,
    Recovery,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub distance: f32,
    pub jump_height: f32,
    pub uprightness: f32,
    pub body_count: f32,
    pub actuator_count: f32,
    pub energy: f32,
}

#[derive(Clone, Debug)]
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
    let mut max_height = 0.0_f32;
    let mut upright_accumulator = 0.0_f32;

    for step in 0..steps {
        if task == TaskKind::Recovery && step == 0 {
            let _ = simulation.snapshot();
        }

        let snapshot = simulation.snapshot();
        let observations = observation_vector(&snapshot, &genome.body);
        let actions = brain.think(&observations, &mut brain_state);
        simulation.step(genome, &actions, 0.05);

        let next = simulation.snapshot();
        max_height = max_height.max(next.root_position.y - next.terrain_height);
        upright_accumulator += (1.0 - next.tilt.abs() / 1.5).clamp(0.0, 1.0);
    }

    let snapshot = simulation.snapshot();
    let distance = snapshot.root_position.x.max(0.0);
    let uprightness = upright_accumulator / steps.max(1) as f32;
    let energy = snapshot.energy_spent;
    let metrics = Metrics {
        distance,
        jump_height: max_height,
        uprightness,
        body_count: genome.body.body_count() as f32,
        actuator_count: genome.body.actuator_count() as f32,
        energy,
    };

    let terrain_multiplier = match task {
        TaskKind::FlatRun => 1.0,
        TaskKind::RoughTerrain => 1.25,
        TaskKind::Recovery => 1.15,
    };
    let fitness = distance * 10.0 * terrain_multiplier + uprightness * 5.0 + max_height * 1.5
        - energy * 0.06
        - metrics.body_count * 0.03;

    Evaluation {
        fitness,
        metrics,
        steps,
    }
}

pub fn observation_vector(snapshot: &Snapshot, body: &BodyGenome) -> Vec<f32> {
    let mut observations = Vec::with_capacity(sensor_count(body));
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
    observations
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

    use super::{TaskKind, observation_vector};

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
