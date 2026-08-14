use crate::genome::{BodyGenome, Genome};
use crate::math::{Attachment, Vec2, child_center_offset, face_anchor};

#[derive(Clone, Copy, Debug)]
pub enum TerrainKind {
    Flat,
    Rough,
    Steps,
}

#[derive(Clone, Copy, Debug)]
pub struct World {
    pub terrain: TerrainKind,
    pub gravity: f32,
    pub friction: f32,
}

impl World {
    pub fn flat() -> Self {
        Self {
            terrain: TerrainKind::Flat,
            gravity: 9.8,
            friction: 0.14,
        }
    }

    pub fn rough() -> Self {
        Self {
            terrain: TerrainKind::Rough,
            gravity: 9.8,
            friction: 0.18,
        }
    }

    pub fn terrain_height(&self, x: f32) -> f32 {
        match self.terrain {
            TerrainKind::Flat => 0.0,
            TerrainKind::Rough => (x * 1.7).sin() * 0.12 + (x * 0.47).cos() * 0.08,
            TerrainKind::Steps => (x / 1.5).floor() * 0.05,
        }
    }

    pub fn terrain_slope(&self, x: f32) -> f32 {
        let left = self.terrain_height(x - 0.05);
        let right = self.terrain_height(x + 0.05);
        (right - left) / 0.1
    }
}

#[derive(Clone, Copy, Debug)]
pub struct JointAnchor {
    pub parent_anchor: Vec2,
    pub child_anchor: Vec2,
}

#[derive(Clone, Debug)]
pub struct JointState {
    pub node_id: usize,
    pub attachment: Attachment,
    pub anchor: JointAnchor,
    pub angle: f32,
    pub angular_velocity: f32,
}

#[derive(Clone, Debug)]
pub struct CreatureInstance {
    pub root_position: Vec2,
    pub root_velocity: Vec2,
    pub tilt: f32,
    pub angular_velocity: f32,
    pub energy_spent: f32,
    pub body_positions: Vec<Vec2>,
    pub joints: Vec<JointState>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub time: f32,
    pub root_position: Vec2,
    pub root_velocity: Vec2,
    pub tilt: f32,
    pub angular_velocity: f32,
    pub energy_spent: f32,
    pub terrain_height: f32,
    pub terrain_slope: f32,
    pub joints: Vec<JointState>,
}

#[derive(Clone, Debug)]
pub struct Simulation {
    pub world: World,
    pub time: f32,
    creature: Option<CreatureInstance>,
}

impl Simulation {
    pub fn new(world: World) -> Self {
        Self {
            world,
            time: 0.0,
            creature: None,
        }
    }

    pub fn spawn_creature(&mut self, body: &BodyGenome) {
        self.time = 0.0;
        self.creature = Some(build_creature(body));
        if let Some(creature) = &mut self.creature {
            creature.root_position.y = self.world.terrain_height(0.0) + 0.6;
        }
    }

    pub fn creature_body_count(&self) -> usize {
        self.creature
            .as_ref()
            .map(|creature| creature.body_positions.len())
            .unwrap_or(0)
    }

    pub fn snapshot(&self) -> Snapshot {
        let creature = self
            .creature
            .as_ref()
            .expect("spawn_creature must be called before snapshot");
        Snapshot {
            time: self.time,
            root_position: creature.root_position,
            root_velocity: creature.root_velocity,
            tilt: creature.tilt,
            angular_velocity: creature.angular_velocity,
            energy_spent: creature.energy_spent,
            terrain_height: self.world.terrain_height(creature.root_position.x),
            terrain_slope: self.world.terrain_slope(creature.root_position.x),
            joints: creature.joints.clone(),
        }
    }

    pub(crate) fn snapshot_into(&self, snapshot: &mut Snapshot) {
        let creature = self
            .creature
            .as_ref()
            .expect("spawn_creature must be called before snapshot");
        snapshot.time = self.time;
        snapshot.root_position = creature.root_position;
        snapshot.root_velocity = creature.root_velocity;
        snapshot.tilt = creature.tilt;
        snapshot.angular_velocity = creature.angular_velocity;
        snapshot.energy_spent = creature.energy_spent;
        snapshot.terrain_height = self.world.terrain_height(creature.root_position.x);
        snapshot.terrain_slope = self.world.terrain_slope(creature.root_position.x);
        snapshot.joints.clone_from(&creature.joints);
    }

    pub fn step(&mut self, genome: &Genome, actions: &[f32], dt: f32) {
        let creature = self
            .creature
            .as_mut()
            .expect("spawn_creature must be called before step");

        let mut thrust = 0.0;
        let mut lift = 0.0;
        let mut torque = 0.0;
        let mut action_index = 0;
        for joint in &mut creature.joints {
            let node = &genome.body.nodes[joint.node_id];
            let action = if node.is_actuated() {
                let action = actions
                    .get(action_index)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(-1.0, 1.0);
                action_index += 1;
                action
            } else {
                0.0
            };
            let target_velocity = action * node.actuator_strength * 3.0;
            joint.angular_velocity += (target_velocity - joint.angular_velocity) * 0.35;
            joint.angle = (joint.angle + joint.angular_velocity * dt).clamp(-1.4, 1.4);

            let normal = joint.attachment.normal();
            thrust += action * node.actuator_strength * normal.x.abs().max(0.35);
            lift += action.abs() * node.actuator_strength * normal.y.max(0.0) * 0.12;
            torque += action * node.actuator_strength * normal.y * 0.05;
            creature.energy_spent += action.abs() * node.actuator_strength * dt;
        }

        let body_scale = genome.body.body_count() as f32;
        creature.root_velocity.x += (thrust / body_scale) * dt;
        creature.root_velocity.y += lift * dt - self.world.gravity * 0.05 * dt;
        creature.root_velocity.x *= 1.0 - self.world.friction * dt;
        creature.root_position += creature.root_velocity * dt;

        let floor = self.world.terrain_height(creature.root_position.x) + 0.35;
        if creature.root_position.y < floor {
            creature.root_position.y = floor;
            creature.root_velocity.y = creature.root_velocity.y.max(0.0);
        }

        creature.angular_velocity +=
            torque - self.world.terrain_slope(creature.root_position.x) * 0.02;
        creature.angular_velocity *= 0.96;
        creature.tilt = (creature.tilt + creature.angular_velocity * dt).clamp(-1.5, 1.5);
        self.time += dt;
    }
}

fn build_creature(body: &BodyGenome) -> CreatureInstance {
    let body_positions = body.positions();
    let mut joints = Vec::new();
    for node in body.nodes.iter().filter(|node| node.parent.is_some()) {
        let parent = &body.nodes[node.parent.unwrap()];
        joints.push(JointState {
            node_id: node.id,
            attachment: node.attachment,
            anchor: JointAnchor {
                parent_anchor: face_anchor(parent.size, node.attachment),
                child_anchor: face_anchor(node.size, node.attachment.opposite()),
            },
            angle: 0.0,
            angular_velocity: 0.0,
        });
    }

    CreatureInstance {
        root_position: Vec2::new(0.0, 0.6),
        root_velocity: Vec2::ZERO,
        tilt: 0.0,
        angular_velocity: 0.0,
        energy_spent: 0.0,
        body_positions,
        joints,
    }
}

pub fn attachment_offset(parent_size: Vec2, child_size: Vec2, attachment: Attachment) -> Vec2 {
    child_center_offset(parent_size, child_size, attachment)
}

#[cfg(test)]
mod tests {
    use crate::{
        Genome,
        math::{Attachment, Vec2},
        policy::ControllerKind,
        rng::Rng,
    };

    use super::{Simulation, World, attachment_offset};

    #[test]
    fn child_placement_respects_attachment_direction() {
        let parent = Vec2::new(2.0, 2.0);
        let child = Vec2::new(0.5, 0.5);

        let right = attachment_offset(parent, child, Attachment::Right);
        let top = attachment_offset(parent, child, Attachment::Top);

        assert!(right.x > 1.0);
        assert!(right.y.abs() < 0.001);
        assert!(top.y > 1.0);
        assert!(top.x.abs() < 0.001);
    }

    #[test]
    fn spawning_replaces_previous_creature() {
        let mut rng = Rng::new(4);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let mut mutated = genome.clone();
        mutated.body.nodes.pop();

        let mut sim = Simulation::new(World::flat());
        sim.spawn_creature(&genome.body);
        assert_eq!(sim.creature_body_count(), genome.body.body_count());

        sim.spawn_creature(&mutated.body);
        assert_eq!(sim.creature_body_count(), mutated.body.body_count());
    }

    #[test]
    fn actions_are_consumed_by_actuated_joints_only() {
        let mut rng = Rng::new(5);
        let mut genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        for node in &mut genome.body.nodes[1..4] {
            node.actuator_strength = 0.0;
        }
        genome.body.nodes[4].actuator_strength = 1.0;

        let mut sim = Simulation::new(World::flat());
        sim.spawn_creature(&genome.body);
        sim.step(&genome, &[1.0], 0.05);

        let snapshot = sim.snapshot();
        assert!(snapshot.root_position.x > 0.0);
        assert!(snapshot.energy_spent > 0.0);
    }

    #[test]
    fn reusable_snapshot_is_identical_to_an_owned_snapshot() {
        let mut rng = Rng::new(17);
        let genome = Genome::minimal(ControllerKind::Cpg, &mut rng);
        let mut sim = Simulation::new(World::rough());
        sim.spawn_creature(&genome.body);
        let mut reusable = sim.snapshot();

        sim.step(&genome, &[0.25, -0.5, 0.75, -1.0], 0.05);
        sim.snapshot_into(&mut reusable);
        let owned = sim.snapshot();

        assert_eq!(reusable.time.to_bits(), owned.time.to_bits());
        assert_eq!(reusable.root_position, owned.root_position);
        assert_eq!(reusable.root_velocity, owned.root_velocity);
        assert_eq!(reusable.tilt.to_bits(), owned.tilt.to_bits());
        assert_eq!(
            reusable.angular_velocity.to_bits(),
            owned.angular_velocity.to_bits()
        );
        assert_eq!(
            reusable.energy_spent.to_bits(),
            owned.energy_spent.to_bits()
        );
        assert_eq!(
            reusable.terrain_height.to_bits(),
            owned.terrain_height.to_bits()
        );
        assert_eq!(
            reusable.terrain_slope.to_bits(),
            owned.terrain_slope.to_bits()
        );
        assert_eq!(reusable.joints.len(), owned.joints.len());
        for (actual, expected) in reusable.joints.iter().zip(&owned.joints) {
            assert_eq!(actual.node_id, expected.node_id);
            assert_eq!(actual.attachment, expected.attachment);
            assert_eq!(actual.anchor.parent_anchor, expected.anchor.parent_anchor);
            assert_eq!(actual.anchor.child_anchor, expected.anchor.child_anchor);
            assert_eq!(actual.angle.to_bits(), expected.angle.to_bits());
            assert_eq!(
                actual.angular_velocity.to_bits(),
                expected.angular_velocity.to_bits()
            );
        }
    }
}
