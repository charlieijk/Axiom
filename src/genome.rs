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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MorphologyConstraints {
    pub max_body_parts: usize,
    pub max_actuators: usize,
    pub min_part_size: Vec2,
    pub max_part_size: Vec2,
    pub max_actuator_strength: f32,
}

impl Default for MorphologyConstraints {
    fn default() -> Self {
        Self {
            max_body_parts: 10,
            max_actuators: 10,
            min_part_size: Vec2::new(0.12, 0.12),
            max_part_size: Vec2::new(1.6, 1.2),
            max_actuator_strength: 1.5,
        }
    }
}

impl MorphologyConstraints {
    pub fn rough_inspection() -> Self {
        Self {
            max_body_parts: 8,
            max_actuators: 6,
            ..Self::default()
        }
    }

    fn normalized(self) -> Self {
        Self {
            max_body_parts: self.max_body_parts.max(1),
            max_actuators: self.max_actuators,
            min_part_size: Vec2::new(
                self.min_part_size.x.max(0.01),
                self.min_part_size.y.max(0.01),
            ),
            max_part_size: Vec2::new(
                self.max_part_size.x.max(self.min_part_size.x.max(0.01)),
                self.max_part_size.y.max(self.min_part_size.y.max(0.01)),
            ),
            max_actuator_strength: self.max_actuator_strength.max(0.0),
        }
    }
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
        self.mutate_with_constraints(rng, &MorphologyConstraints::default());
    }

    pub fn mutate_with_constraints(&mut self, rng: &mut Rng, constraints: &MorphologyConstraints) {
        let constraints = constraints.normalized();
        for node in &mut self.nodes {
            if node.parent.is_some() && rng.chance(0.2) {
                node.actuator_strength = (node.actuator_strength + rng.range_f32(-0.2, 0.2))
                    .clamp(0.0, constraints.max_actuator_strength);
            }
            if rng.chance(0.1) {
                node.size.x = (node.size.x + rng.range_f32(-0.08, 0.08))
                    .clamp(constraints.min_part_size.x, constraints.max_part_size.x);
                node.size.y = (node.size.y + rng.range_f32(-0.08, 0.08))
                    .clamp(constraints.min_part_size.y, constraints.max_part_size.y);
            }
        }

        if self.nodes.len() < constraints.max_body_parts && rng.chance(0.08) {
            let parent = rng.range_usize(self.nodes.len());
            let attachment = match rng.range_usize(4) {
                0 => Attachment::Right,
                1 => Attachment::Left,
                2 => Attachment::Top,
                _ => Attachment::Bottom,
            };
            let actuator_strength = if self.actuator_count() < constraints.max_actuators {
                rng.range_f32(0.25, 1.2)
                    .clamp(0.0, constraints.max_actuator_strength)
            } else {
                0.0
            };
            self.nodes.push(BodyNode {
                id: self.nodes.len(),
                parent: Some(parent),
                attachment,
                size: Vec2::new(
                    rng.range_f32(0.18, 0.7)
                        .clamp(constraints.min_part_size.x, constraints.max_part_size.x),
                    rng.range_f32(0.16, 0.65)
                        .clamp(constraints.min_part_size.y, constraints.max_part_size.y),
                ),
                actuator_strength,
            });
        }

        self.repair_constraints(&constraints);
    }

    pub fn repair_constraints(&mut self, constraints: &MorphologyConstraints) {
        let constraints = constraints.normalized();
        self.nodes.truncate(constraints.max_body_parts);

        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.id = index;
            if index == 0 {
                node.parent = None;
                node.actuator_strength = 0.0;
            } else if node.parent.is_none_or(|parent| parent >= index) {
                node.parent = Some(0);
            }
            node.size.x = node
                .size
                .x
                .clamp(constraints.min_part_size.x, constraints.max_part_size.x);
            node.size.y = node
                .size
                .y
                .clamp(constraints.min_part_size.y, constraints.max_part_size.y);
            node.actuator_strength = node
                .actuator_strength
                .clamp(0.0, constraints.max_actuator_strength);
        }

        let mut actuators_seen = 0_usize;
        for node in &mut self.nodes {
            if node.is_actuated() {
                if actuators_seen >= constraints.max_actuators {
                    node.actuator_strength = 0.0;
                } else {
                    actuators_seen += 1;
                }
            }
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
        self.mutate_with_constraints(rng, &MorphologyConstraints::default())
    }

    pub fn mutate_with_constraints(
        &self,
        rng: &mut Rng,
        constraints: &MorphologyConstraints,
    ) -> Self {
        let mut next = self.clone();
        next.body.mutate_with_constraints(rng, constraints);
        next.brain.mutate_weights(rng, 0.35, 0.25);

        if rng.chance(0.04) {
            next.controller = match rng.range_usize(3) {
                0 => ControllerKind::FeedForward,
                1 => ControllerKind::Recurrent,
                _ => ControllerKind::Cpg,
            };
        }

        next.resize_brain_for_body(rng);

        next
    }

    pub fn enforce_morphology_constraints(
        &mut self,
        constraints: &MorphologyConstraints,
        rng: &mut Rng,
    ) {
        self.body.repair_constraints(constraints);
        self.resize_brain_for_body(rng);
    }

    fn resize_brain_for_body(&mut self, rng: &mut Rng) {
        if self.brain.output_count != self.body.output_count()
            || self.brain.input_count != controller_input_count(&self.body, self.controller)
        {
            self.brain = self.brain.resized_preserving(
                controller_input_count(&self.body, self.controller),
                self.body.output_count(),
                rng,
            );
        }
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
    use super::{
        Genome, MorphologyConstraints, NeuralGenome, controller_input_count, sensor_count,
    };
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

    #[test]
    fn morphology_constraints_limit_body_and_actuators() {
        let mut rng = Rng::new(51);
        let mut genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let constraints = MorphologyConstraints {
            max_body_parts: 3,
            max_actuators: 1,
            ..MorphologyConstraints::default()
        };

        genome.enforce_morphology_constraints(&constraints, &mut rng);

        assert_eq!(genome.body.body_count(), 3);
        assert_eq!(genome.body.actuator_count(), 1);
        assert_eq!(genome.brain.output_count, 1);
        assert_eq!(
            genome.brain.input_count,
            controller_input_count(&genome.body, genome.controller)
        );
    }

    #[test]
    fn constrained_mutation_keeps_brain_sized_to_body() {
        let mut rng = Rng::new(52);
        let genome = Genome::minimal(ControllerKind::Recurrent, &mut rng);
        let constraints = MorphologyConstraints {
            max_body_parts: 4,
            max_actuators: 2,
            ..MorphologyConstraints::default()
        };

        let mutated = genome.mutate_with_constraints(&mut rng, &constraints);

        assert!(mutated.body.body_count() <= 4);
        assert!(mutated.body.actuator_count() <= 2);
        assert_eq!(mutated.brain.output_count, mutated.body.output_count());
        assert_eq!(
            mutated.brain.input_count,
            controller_input_count(&mutated.body, mutated.controller)
        );
    }
}
