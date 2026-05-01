use std::collections::HashMap;

use crate::genome::{NeuralGenome, NodeKind};

#[derive(Clone, Debug)]
struct RuntimeNode {
    id: usize,
    kind: NodeKind,
}

#[derive(Clone, Debug)]
pub struct CompiledNetwork {
    nodes: Vec<RuntimeNode>,
    incoming: Vec<Vec<(usize, f32)>>,
    input_slots: Vec<usize>,
    bias_slots: Vec<usize>,
    output_slots: Vec<usize>,
    order: Vec<usize>,
}

impl CompiledNetwork {
    pub fn compile(genome: &NeuralGenome) -> Self {
        let mut nodes: Vec<_> = genome
            .nodes
            .iter()
            .map(|node| RuntimeNode {
                id: node.id,
                kind: node.kind,
            })
            .collect();
        nodes.sort_by_key(|node| node.id);

        let index_by_id: HashMap<usize, usize> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, index))
            .collect();

        let mut incoming = vec![Vec::new(); nodes.len()];
        let mut outgoing = vec![Vec::new(); nodes.len()];
        for connection in genome
            .connections
            .iter()
            .filter(|connection| connection.enabled)
        {
            if let (Some(&from), Some(&to)) = (
                index_by_id.get(&connection.from),
                index_by_id.get(&connection.to),
            ) {
                incoming[to].push((from, connection.weight));
                outgoing[from].push(to);
            }
        }

        let order = topological_order(&nodes, &incoming, &outgoing);
        let mut input_slots = slots_for_kind(&nodes, NodeKind::Input);
        let bias_slots = slots_for_kind(&nodes, NodeKind::Bias);
        let output_slots = slots_for_kind(&nodes, NodeKind::Output);

        input_slots.sort_by_key(|slot| nodes[*slot].id);

        Self {
            nodes,
            incoming,
            input_slots,
            bias_slots,
            output_slots,
            order,
        }
    }

    pub fn input_count(&self) -> usize {
        self.input_slots.len()
    }

    pub fn output_count(&self) -> usize {
        self.output_slots.len()
    }

    pub fn forward(&self, inputs: &[f32]) -> Vec<f32> {
        let mut values = vec![0.0; self.nodes.len()];

        for (slot_index, node_index) in self.input_slots.iter().copied().enumerate() {
            values[node_index] = inputs.get(slot_index).copied().unwrap_or(0.0);
        }

        for node_index in &self.bias_slots {
            values[*node_index] = 1.0;
        }

        for node_index in &self.order {
            match self.nodes[*node_index].kind {
                NodeKind::Input | NodeKind::Bias => {}
                NodeKind::Hidden | NodeKind::Output => {
                    let sum = self.incoming[*node_index]
                        .iter()
                        .map(|(from, weight)| values[*from] * weight)
                        .sum::<f32>();
                    values[*node_index] = sum.tanh();
                }
            }
        }

        self.output_slots
            .iter()
            .map(|node_index| values[*node_index])
            .collect()
    }
}

fn slots_for_kind(nodes: &[RuntimeNode], kind: NodeKind) -> Vec<usize> {
    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (node.kind == kind).then_some(index))
        .collect()
}

fn topological_order(
    nodes: &[RuntimeNode],
    incoming: &[Vec<(usize, f32)>],
    outgoing: &[Vec<usize>],
) -> Vec<usize> {
    let mut indegree: Vec<usize> = incoming.iter().map(Vec::len).collect();
    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    ready.sort_by_key(|index| nodes[*index].id);

    let mut order = Vec::with_capacity(nodes.len());
    while let Some(index) = ready.first().copied() {
        ready.remove(0);
        order.push(index);
        for to in &outgoing[index] {
            indegree[*to] = indegree[*to].saturating_sub(1);
            if indegree[*to] == 0 {
                ready.push(*to);
                ready.sort_by_key(|slot| nodes[*slot].id);
            }
        }
    }

    if order.len() != nodes.len() {
        let mut fallback: Vec<usize> = (0..nodes.len()).collect();
        fallback.sort_by_key(|index| nodes[*index].id);
        return fallback;
    }

    order
}

#[cfg(test)]
mod tests {
    use crate::genome::{ConnectionGene, NeuralGenome, NodeGene, NodeKind};

    use super::CompiledNetwork;

    #[test]
    fn bias_nodes_do_not_consume_external_inputs() {
        let genome = NeuralGenome {
            input_count: 2,
            output_count: 1,
            nodes: vec![
                NodeGene {
                    id: 2,
                    kind: NodeKind::Bias,
                },
                NodeGene {
                    id: 0,
                    kind: NodeKind::Input,
                },
                NodeGene {
                    id: 1,
                    kind: NodeKind::Input,
                },
                NodeGene {
                    id: 3,
                    kind: NodeKind::Output,
                },
            ],
            connections: vec![
                ConnectionGene {
                    from: 0,
                    to: 3,
                    weight: 1.0,
                    enabled: true,
                },
                ConnectionGene {
                    from: 1,
                    to: 3,
                    weight: 10.0,
                    enabled: true,
                },
                ConnectionGene {
                    from: 2,
                    to: 3,
                    weight: 100.0,
                    enabled: true,
                },
            ],
        };

        let network = CompiledNetwork::compile(&genome);
        let output = network.forward(&[0.25, 0.5])[0];
        let expected = (0.25_f32 + 5.0 + 100.0).tanh();
        assert!((output - expected).abs() < 0.0001);
    }
}
