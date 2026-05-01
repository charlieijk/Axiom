use crate::math::{Attachment, Vec2, child_center_offset};
use crate::policy::ControllerKind;
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Input,
    Bias,
    Hidden,
    Output,
}

#[derive(Clone, Debug)]
pub struct NodeGene {
    pub id: usize,
    pub kind: NodeKind,
}

#[derive(Clone, Debug)]
pub struct ConnectionGene {
    pub from: usize,
    pub to: usize,
    pub weight: f32,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct NeuralGenome {
    pub input_count: usize,
    pub output_count: usize,
    pub nodes: Vec<NodeGene>,
    pub connections: Vec<ConnectionGene>,
}

impl NeuralGenome {
    pub fn minimal(input_count: usize, output_count: usize, rng: &mut Rng) -> Self {
        let mut nodes = Vec::with_capacity(input_count + output_count + 1);
        for id in 0..input_count {
            nodes.push(NodeGene {
                id,
                kind: NodeKind::Input,
            });
        }

        let bias_id = input_count;
        nodes.push(NodeGene {
            id: bias_id,
            kind: NodeKind::Bias,
        });

        let output_start = input_count + 1;
        for offset in 0..output_count {
            nodes.push(NodeGene {
                id: output_start + offset,
                kind: NodeKind::Output,
            });
        }

        let mut connections = Vec::new();
        for source in 0..=input_count {
            for output in 0..output_count {
                connections.push(ConnectionGene {
                    from: source,
                    to: output_start + output,
                    weight: rng.range_f32(-1.0, 1.0),
                    enabled: true,
                });
            }
        }

        Self {
            input_count,
            output_count,
            nodes,
            connections,
        }
    }

    pub fn mutate_weights(&mut self, rng: &mut Rng, rate: f32, scale: f32) {
        for connection in &mut self.connections {
            if rng.chance(rate) {
                connection.weight =
                    (connection.weight + rng.range_f32(-scale, scale)).clamp(-3.0, 3.0);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BodyNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub attachment: Attachment,
    pub size: Vec2,
    pub actuator_strength: f32,
}

impl BodyNode {
    pub fn is_actuated(&self) -> bool {
        self.parent.is_some() && self.actuator_strength.abs() > 0.01
    }
}

#[derive(Clone, Debug)]
pub struct BodyGenome {
    pub nodes: Vec<BodyNode>,
}

impl BodyGenome {
    pub fn seed_quadruped() -> Self {
        let root = BodyNode {
            id: 0,
            parent: None,
            attachment: Attachment::Right,
            size: Vec2::new(1.2, 0.55),
            actuator_strength: 0.0,
        };

        let limbs = [
            (Attachment::Left, Vec2::new(0.45, 0.2), 0.9),
            (Attachment::Right, Vec2::new(0.45, 0.2), 0.9),
            (Attachment::Top, Vec2::new(0.22, 0.45), 0.5),
            (Attachment::Bottom, Vec2::new(0.22, 0.55), 1.0),
        ];

        let mut nodes = vec![root];
        for (index, (attachment, size, strength)) in limbs.into_iter().enumerate() {
            nodes.push(BodyNode {
                id: index + 1,
                parent: Some(0),
                attachment,
                size,
                actuator_strength: strength,
            });
        }

        Self { nodes }
    }

    pub fn body_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn actuator_count(&self) -> usize {
        self.nodes.iter().filter(|node| node.is_actuated()).count()
    }

    pub fn output_count(&self) -> usize {
        self.actuator_count().max(1)
    }

    pub fn positions(&self) -> Vec<Vec2> {
        let mut positions = vec![Vec2::ZERO; self.nodes.len()];
        for node in &self.nodes {
            if let Some(parent_id) = node.parent {
                let parent = &self.nodes[parent_id];
                positions[node.id] = positions[parent_id]
                    + child_center_offset(parent.size, node.size, node.attachment);
            }
        }
        positions
    }

    pub fn mutate(&mut self, rng: &mut Rng) {
        for node in &mut self.nodes {
            if node.parent.is_some() && rng.chance(0.2) {
                node.actuator_strength =
                    (node.actuator_strength + rng.range_f32(-0.2, 0.2)).clamp(0.0, 1.5);
            }
            if rng.chance(0.1) {
                node.size.x = (node.size.x + rng.range_f32(-0.08, 0.08)).clamp(0.12, 1.6);
                node.size.y = (node.size.y + rng.range_f32(-0.08, 0.08)).clamp(0.12, 1.2);
            }
        }

        if self.nodes.len() < 10 && rng.chance(0.08) {
            let parent = rng.range_usize(self.nodes.len());
            let attachment = match rng.range_usize(4) {
                0 => Attachment::Right,
                1 => Attachment::Left,
                2 => Attachment::Top,
                _ => Attachment::Bottom,
            };
            self.nodes.push(BodyNode {
                id: self.nodes.len(),
                parent: Some(parent),
                attachment,
                size: Vec2::new(rng.range_f32(0.18, 0.7), rng.range_f32(0.16, 0.65)),
                actuator_strength: rng.range_f32(0.25, 1.2),
            });
        }
    }
}

#[derive(Clone, Debug)]
pub struct Genome {
    pub body: BodyGenome,
    pub brain: NeuralGenome,
    pub controller: ControllerKind,
}

impl Genome {
    pub fn minimal(controller: ControllerKind, rng: &mut Rng) -> Self {
        let body = BodyGenome::seed_quadruped();
        let brain = NeuralGenome::minimal(sensor_count(&body), body.output_count(), rng);
        Self {
            body,
            brain,
            controller,
        }
    }

    pub fn mutate(&self, rng: &mut Rng) -> Self {
        let mut next = self.clone();
        next.body.mutate(rng);
        next.brain.mutate_weights(rng, 0.35, 0.25);

        if next.brain.output_count != next.body.output_count()
            || next.brain.input_count != sensor_count(&next.body)
        {
            next.brain =
                NeuralGenome::minimal(sensor_count(&next.body), next.body.output_count(), rng);
        }

        if rng.chance(0.04) {
            next.controller = match rng.range_usize(3) {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            };
        }

        next
    }
}

pub fn sensor_count(body: &BodyGenome) -> usize {
    12 + body.actuator_count() * 2 + body.body_count() * 2
}
