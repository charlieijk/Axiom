use crate::math::{Attachment, Vec2, child_center_offset};
use crate::policy::ControllerKind;
use crate::rng::Rng;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NodeKind {
    Input,
    Bias,
    Hidden,
    Output,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeGene {
    pub id: usize,
    pub kind: NodeKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionGene {
    pub from: usize,
    pub to: usize,
    pub weight: f32,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

    pub fn resized_preserving(
        &self,
        input_count: usize,
        output_count: usize,
        rng: &mut Rng,
    ) -> Self {
        let preserved = self.direct_output_connections();
        let mut next = Self::minimal(input_count, output_count, rng);

        for connection in &mut next.connections {
            let Some(source) = new_minimal_source(connection.from, input_count) else {
                continue;
            };
            let Some(output_index) = new_minimal_output_index(connection.to, input_count) else {
                continue;
            };

            if let Some(preserved) = preserved.iter().find(|candidate| {
                candidate.source == source && candidate.output_index == output_index
            }) {
                connection.weight = preserved.weight;
                connection.enabled = preserved.enabled;
            }
        }

        next
    }

    fn direct_output_connections(&self) -> Vec<PreservedConnection> {
        let input_ids = sorted_node_ids(&self.nodes, NodeKind::Input);
        let bias_ids = sorted_node_ids(&self.nodes, NodeKind::Bias);
        let output_ids = sorted_node_ids(&self.nodes, NodeKind::Output);

        self.connections
            .iter()
            .filter_map(|connection| {
                let source = if let Some(index) = input_ids
                    .iter()
                    .position(|node_id| *node_id == connection.from)
                {
                    PreservedSource::Input(index)
                } else if bias_ids.contains(&connection.from) {
                    PreservedSource::Bias
                } else {
                    return None;
                };

                let output_index = output_ids
                    .iter()
                    .position(|node_id| *node_id == connection.to)?;

                Some(PreservedConnection {
                    source,
                    output_index,
                    weight: connection.weight,
                    enabled: connection.enabled,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreservedSource {
    Input(usize),
    Bias,
}

#[derive(Clone, Copy, Debug)]
struct PreservedConnection {
    source: PreservedSource,
    output_index: usize,
    weight: f32,
    enabled: bool,
}

fn sorted_node_ids(nodes: &[NodeGene], kind: NodeKind) -> Vec<usize> {
    let mut ids: Vec<_> = nodes
        .iter()
        .filter_map(|node| (node.kind == kind).then_some(node.id))
        .collect();
    ids.sort_unstable();
    ids
}

fn new_minimal_source(node_id: usize, input_count: usize) -> Option<PreservedSource> {
    if node_id < input_count {
        Some(PreservedSource::Input(node_id))
    } else if node_id == input_count {
        Some(PreservedSource::Bias)
    } else {
        None
    }
}

fn new_minimal_output_index(node_id: usize, input_count: usize) -> Option<usize> {
    node_id.checked_sub(input_count + 1)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

    pub fn actuated_nodes(&self) -> impl Iterator<Item = &BodyNode> {
        self.nodes.iter().filter(|node| node.is_actuated())
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Genome {
    pub body: BodyGenome,
    pub brain: NeuralGenome,
    pub controller: ControllerKind,
}

impl Genome {
    pub fn minimal(controller: ControllerKind, rng: &mut Rng) -> Self {
        let body = BodyGenome::seed_quadruped();
        let brain = NeuralGenome::minimal(
            controller_input_count(&body, controller),
            body.output_count(),
            rng,
        );
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

        if rng.chance(0.04) {
            next.controller = match rng.range_usize(3) {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            };
        }

        if next.brain.output_count != next.body.output_count()
            || next.brain.input_count != controller_input_count(&next.body, next.controller)
        {
            next.brain = next.brain.resized_preserving(
                controller_input_count(&next.body, next.controller),
                next.body.output_count(),
                rng,
            );
        }

        next
    }
}

pub fn sensor_count(body: &BodyGenome) -> usize {
    12 + body.actuator_count() * 2 + body.body_count() * 2
}

pub fn controller_input_count(body: &BodyGenome, controller: ControllerKind) -> usize {
    let base = sensor_count(body);
    if controller == ControllerKind::Recurrent {
        base + body.output_count()
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::{Genome, NeuralGenome, controller_input_count, sensor_count};
    use crate::{policy::ControllerKind, rng::Rng};

    #[test]
    fn recurrent_genomes_reserve_inputs_for_prior_outputs() {
        let mut rng = Rng::new(31);
        let genome = Genome::minimal(ControllerKind::Recurrent, &mut rng);

        assert_eq!(
            genome.brain.input_count,
            sensor_count(&genome.body) + genome.body.output_count()
        );
        assert_eq!(
            genome.brain.input_count,
            controller_input_count(&genome.body, genome.controller)
        );
    }

    #[test]
    fn resized_brain_preserves_overlapping_direct_weights() {
        let mut rng = Rng::new(44);
        let mut brain = NeuralGenome::minimal(3, 2, &mut rng);
        for connection in &mut brain.connections {
            if connection.from == 0 && connection.to == 4 {
                connection.weight = 1.25;
            }
            if connection.from == 3 && connection.to == 5 {
                connection.weight = -0.75;
                connection.enabled = false;
            }
        }

        let resized = brain.resized_preserving(5, 3, &mut rng);

        assert_eq!(resized.input_count, 5);
        assert_eq!(resized.output_count, 3);
        assert!(
            resized
                .connections
                .iter()
                .any(|connection| connection.from == 0
                    && connection.to == 6
                    && (connection.weight - 1.25).abs() < 0.0001)
        );
        assert!(
            resized
                .connections
                .iter()
                .any(|connection| connection.from == 5
                    && connection.to == 7
                    && (connection.weight + 0.75).abs() < 0.0001
                    && !connection.enabled)
        );
    }
}
