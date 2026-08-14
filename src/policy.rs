use crate::genome::Genome;
use crate::network::{CompiledNetwork, NetworkWorkspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ControllerKind {
    FeedForward,
    Recurrent,
    Cpg,
}

impl ControllerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FeedForward => "feedforward",
            Self::Recurrent => "recurrent",
            Self::Cpg => "cpg",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrainState {
    pub recurrent: Vec<f32>,
    pub oscillator_phase: f32,
}

impl BrainState {
    pub fn new(output_count: usize) -> Self {
        Self {
            recurrent: vec![0.0; output_count],
            oscillator_phase: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.recurrent.fill(0.0);
        self.oscillator_phase = 0.0;
    }
}

#[derive(Clone, Debug)]
pub struct Brain {
    kind: ControllerKind,
    network: CompiledNetwork,
}

#[derive(Clone, Debug)]
pub(crate) struct BrainWorkspace {
    inputs: Vec<f32>,
    network: NetworkWorkspace,
}

impl Brain {
    pub fn from_genome(genome: &Genome) -> Self {
        Self {
            kind: genome.controller,
            network: CompiledNetwork::compile(&genome.brain),
        }
    }

    pub fn reset_state(&self) -> BrainState {
        BrainState::new(self.network.output_count())
    }

    pub fn think(&self, observations: &[f32], state: &mut BrainState) -> Vec<f32> {
        let mut workspace = self.workspace();
        let _ = self.think_into(observations, state, &mut workspace);
        workspace.network.into_outputs()
    }

    pub(crate) fn workspace(&self) -> BrainWorkspace {
        BrainWorkspace {
            inputs: Vec::with_capacity(self.network.input_count()),
            network: self.network.workspace(),
        }
    }

    pub(crate) fn think_into<'a>(
        &self,
        observations: &[f32],
        state: &mut BrainState,
        workspace: &'a mut BrainWorkspace,
    ) -> &'a [f32] {
        workspace.inputs.clear();
        workspace.inputs.extend_from_slice(observations);

        if self.kind == ControllerKind::Recurrent {
            workspace.inputs.extend_from_slice(&state.recurrent);
        }

        workspace.inputs.resize(self.network.input_count(), 0.0);
        let outputs = self
            .network
            .forward_into(&workspace.inputs, &mut workspace.network);

        match self.kind {
            ControllerKind::FeedForward => {}
            ControllerKind::Recurrent => {
                state.recurrent.clear();
                state.recurrent.extend(outputs.iter().copied());
            }
            ControllerKind::Cpg => {
                let wave = state.oscillator_phase.sin();
                state.oscillator_phase += 0.18;
                for (index, output) in outputs.iter_mut().enumerate() {
                    let phase = if index % 2 == 0 { wave } else { -wave };
                    *output = (*output + phase * 0.65).tanh();
                }
            }
        }

        outputs
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Genome,
        genome::{ConnectionGene, NeuralGenome, NodeGene, NodeKind, sensor_count},
        rng::Rng,
    };

    use super::{Brain, ControllerKind};

    #[test]
    fn recurrent_brain_carries_state_between_steps() {
        let mut rng = Rng::new(7);
        let genome = Genome::minimal(ControllerKind::Recurrent, &mut rng);
        let brain = Brain::from_genome(&genome);
        let mut state = brain.reset_state();
        let observations = vec![0.0; sensor_count(&genome.body)];

        let first = brain.think(&observations, &mut state);
        let second = brain.think(&observations, &mut state);

        assert_eq!(first.len(), second.len());
        assert!(state.recurrent.iter().any(|value| value.abs() > 0.0));
    }

    #[test]
    fn recurrent_brain_uses_prior_outputs_as_inputs() {
        let genome = Genome {
            body: crate::BodyGenome::seed_quadruped(),
            controller: ControllerKind::Recurrent,
            brain: NeuralGenome {
                input_count: 2,
                output_count: 1,
                nodes: vec![
                    NodeGene {
                        id: 0,
                        kind: NodeKind::Input,
                    },
                    NodeGene {
                        id: 1,
                        kind: NodeKind::Input,
                    },
                    NodeGene {
                        id: 2,
                        kind: NodeKind::Bias,
                    },
                    NodeGene {
                        id: 3,
                        kind: NodeKind::Output,
                    },
                ],
                connections: vec![ConnectionGene {
                    from: 1,
                    to: 3,
                    weight: 2.0,
                    enabled: true,
                }],
            },
        };
        let brain = Brain::from_genome(&genome);
        let mut state = brain.reset_state();
        state.recurrent[0] = 0.5;

        let output = brain.think(&[0.0], &mut state);

        assert!((output[0] - 1.0_f32.tanh()).abs() < 0.0001);
    }

    #[test]
    fn reusable_workspace_matches_public_think_for_every_controller() {
        for controller in [
            ControllerKind::FeedForward,
            ControllerKind::Recurrent,
            ControllerKind::Cpg,
        ] {
            let mut rng = Rng::new(41);
            let genome = Genome::minimal(controller, &mut rng);
            let brain = Brain::from_genome(&genome);
            let observations = vec![0.125; sensor_count(&genome.body)];
            let mut expected_state = brain.reset_state();
            let mut actual_state = expected_state.clone();
            let mut workspace = brain.workspace();

            for _ in 0..8 {
                let expected = brain.think(&observations, &mut expected_state);
                let actual = brain
                    .think_into(&observations, &mut actual_state, &mut workspace)
                    .to_vec();

                assert_eq!(actual, expected, "{controller:?} output diverged");
                assert_eq!(actual_state.recurrent, expected_state.recurrent);
                assert_eq!(
                    actual_state.oscillator_phase.to_bits(),
                    expected_state.oscillator_phase.to_bits()
                );
            }
        }
    }
}
