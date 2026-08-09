//! The evolvable controller: one oscillator per joint.
//!
//! This is a central pattern generator, not a neural network, and the choice
//! is deliberate. Axiom's `CompiledNetwork` maps observations to actions, but
//! an open-loop gait has no observations worth mapping — a feedforward net fed
//! only a phase signal *is* a CPG, with extra parameters and worse locality
//! under mutation. Closed-loop control that reacts to tilt and height is a
//! real extension, and it belongs after there is evidence the open-loop case
//! transfers at all.
//!
//! The reference trot from `gait.rs` is one point in this space, which makes
//! it a legitimate seed: the search starts from a gait known to walk rather
//! than from noise.

use axiom::rng::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    gait::Gait,
    sim::{ACTUATOR_COUNT, LEG_COUNT},
};

/// Per-joint oscillator. Amplitude and bias are fractions of servo travel;
/// phase is in cycles.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct JointOscillator {
    pub amplitude: f32,
    pub phase: f32,
    pub bias: f32,
}

pub const MIN_FREQUENCY_HZ: f32 = 0.25;
pub const MAX_FREQUENCY_HZ: f32 = 4.0;
/// Amplitude and bias are capped well below full travel: a command that
/// saturates the servo stops being a sine and starts being a square wave,
/// which is a different controller than the one being scored.
pub const MAX_AMPLITUDE: f32 = 0.6;
pub const MAX_BIAS: f32 = 0.4;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CpgGenome {
    pub frequency_hz: f32,
    pub joints: Vec<JointOscillator>,
}

/// Normalizes a phase into `[0, 1)`.
///
/// Phase is an angle, so 1.0 and 0.0 are the same gait; keeping one canonical
/// representative means a phase can be compared and bounded meaningfully.
/// `rem_euclid` can round up to exactly 1.0 for tiny negative inputs, so the
/// upper end is folded explicitly.
pub fn wrap_phase(phase: f32) -> f32 {
    if !phase.is_finite() {
        return 0.0;
    }
    let wrapped = phase.rem_euclid(1.0);
    if wrapped >= 1.0 { 0.0 } else { wrapped }
}

impl CpgGenome {
    /// The reference trot expressed as a genome, so the known-good gait is a
    /// point the search can start from and must be able to rediscover.
    pub fn from_gait(gait: &Gait) -> Self {
        let mut joints = Vec::with_capacity(ACTUATOR_COUNT);
        for leg in 0..LEG_COUNT {
            let diagonal_phase = if leg == 0 || leg == 3 { 0.0 } else { 0.5 };
            joints.push(JointOscillator {
                amplitude: gait.hip_amplitude,
                phase: wrap_phase(diagonal_phase),
                bias: 0.0,
            });
            joints.push(JointOscillator {
                amplitude: gait.knee_amplitude,
                phase: wrap_phase(diagonal_phase + gait.knee_phase_offset),
                bias: 0.0,
            });
        }
        Self {
            frequency_hz: gait.frequency_hz,
            joints,
        }
    }

    pub fn random(rng: &mut Rng) -> Self {
        let joints = (0..ACTUATOR_COUNT)
            .map(|_| JointOscillator {
                amplitude: rng.range_f32(0.0, MAX_AMPLITUDE),
                phase: rng.next_f32(),
                bias: rng.range_f32(-MAX_BIAS, MAX_BIAS),
            })
            .collect();
        Self {
            frequency_hz: rng.range_f32(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ),
            joints,
        }
    }

    pub fn mutate(&self, rng: &mut Rng, scale: f32) -> Self {
        let mut next = self.clone();
        if rng.chance(0.3) {
            next.frequency_hz = (next.frequency_hz + rng.range_f32(-scale, scale) * 2.0)
                .clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ);
        }
        for joint in &mut next.joints {
            if rng.chance(0.35) {
                joint.amplitude =
                    (joint.amplitude + rng.range_f32(-scale, scale)).clamp(0.0, MAX_AMPLITUDE);
            }
            if rng.chance(0.35) {
                // Phase wraps rather than clamping: clamping would pile
                // genomes up against artificial walls at 0 and 1.
                joint.phase = wrap_phase(joint.phase + rng.range_f32(-scale, scale));
            }
            if rng.chance(0.25) {
                joint.bias = (joint.bias + rng.range_f32(-scale, scale)).clamp(-MAX_BIAS, MAX_BIAS);
            }
        }
        next
    }

    /// Actions for one control tick, in actuator order.
    pub fn actions_at(&self, time_s: f32) -> Vec<f32> {
        self.joints
            .iter()
            .map(|joint| {
                let cycles = time_s * self.frequency_hz + joint.phase;
                (joint.bias + joint.amplitude * (cycles * std::f32::consts::TAU).sin())
                    .clamp(-1.0, 1.0)
            })
            .collect()
    }

    /// True when the genome is structurally usable by the simulator.
    pub fn is_well_formed(&self) -> bool {
        self.joints.len() == ACTUATOR_COUNT
            && self.frequency_hz.is_finite()
            && self.joints.iter().all(|joint| {
                joint.amplitude.is_finite() && joint.phase.is_finite() && joint.bias.is_finite()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CpgGenome, MAX_AMPLITUDE, MAX_BIAS, MAX_FREQUENCY_HZ, MIN_FREQUENCY_HZ, wrap_phase,
    };
    use crate::{gait::sine_gait, sim::ACTUATOR_COUNT};
    use axiom::rng::Rng;

    #[test]
    fn the_reference_trot_survives_the_round_trip_into_a_genome() {
        // If the seed genome did not reproduce the reference gait exactly, the
        // search would be starting somewhere other than where it claims to.
        let gait = sine_gait();
        let genome = CpgGenome::from_gait(&gait);

        for tick in 0..80 {
            let time_s = tick as f32 * 0.05;
            let from_gait = gait.actions_at(time_s);
            let from_genome = genome.actions_at(time_s);
            for (a, b) in from_gait.iter().zip(from_genome.iter()) {
                assert!((a - b).abs() < 1e-6, "at {time_s}s: {a} vs {b}");
            }
        }
    }

    #[test]
    fn the_seed_genome_carries_canonical_phases() {
        // The trot's knee phases land on exactly 1.0 before wrapping, which is
        // the same gait as 0.0 but would sit outside every bound the search
        // relies on.
        for joint in &CpgGenome::from_gait(&sine_gait()).joints {
            assert!((0.0..1.0).contains(&joint.phase), "phase {}", joint.phase);
        }
    }

    #[test]
    fn wrapping_folds_both_ends_and_survives_nonsense() {
        assert_eq!(wrap_phase(1.0), 0.0);
        assert_eq!(wrap_phase(0.0), 0.0);
        assert!((wrap_phase(1.25) - 0.25).abs() < 1e-6);
        assert!((wrap_phase(-0.25) - 0.75).abs() < 1e-6);
        assert_eq!(wrap_phase(f32::NAN), 0.0);
    }

    #[test]
    fn a_random_genome_is_well_formed_and_in_range() {
        let mut rng = Rng::new(19);
        for _ in 0..200 {
            let genome = CpgGenome::random(&mut rng);
            assert!(genome.is_well_formed());
            assert_eq!(genome.joints.len(), ACTUATOR_COUNT);
            assert!((MIN_FREQUENCY_HZ..=MAX_FREQUENCY_HZ).contains(&genome.frequency_hz));
            for joint in &genome.joints {
                assert!((0.0..=MAX_AMPLITUDE).contains(&joint.amplitude));
                assert!(joint.bias.abs() <= MAX_BIAS);
            }
        }
    }

    #[test]
    fn mutation_stays_inside_every_bound() {
        let mut rng = Rng::new(23);
        let mut genome = CpgGenome::from_gait(&sine_gait());

        for _ in 0..500 {
            genome = genome.mutate(&mut rng, 0.3);
            assert!(genome.is_well_formed());
            assert!((MIN_FREQUENCY_HZ..=MAX_FREQUENCY_HZ).contains(&genome.frequency_hz));
            for joint in &genome.joints {
                assert!((0.0..=MAX_AMPLITUDE).contains(&joint.amplitude));
                assert!(joint.bias.abs() <= MAX_BIAS);
                assert!((0.0..1.0).contains(&joint.phase), "phase {}", joint.phase);
            }
        }
    }

    #[test]
    fn phase_wraps_rather_than_piling_up_at_a_wall() {
        let mut rng = Rng::new(29);
        let mut genome = CpgGenome::from_gait(&sine_gait());
        genome.joints[0].phase = 0.99;

        let mut wrapped = false;
        for _ in 0..400 {
            genome = genome.mutate(&mut rng, 0.4);
            if genome.joints[0].phase < 0.2 {
                wrapped = true;
                break;
            }
        }

        assert!(wrapped, "phase never wrapped past 1.0");
    }

    #[test]
    fn commands_never_leave_the_unit_range() {
        let mut rng = Rng::new(31);
        for _ in 0..100 {
            let genome = CpgGenome::random(&mut rng);
            for tick in 0..60 {
                for action in genome.actions_at(tick as f32 * 0.05) {
                    assert!((-1.0..=1.0).contains(&action));
                }
            }
        }
    }

    #[test]
    fn mutation_is_reproducible_from_a_seed() {
        let genome = CpgGenome::from_gait(&sine_gait());
        let run = || {
            let mut rng = Rng::new(101);
            let mut current = genome.clone();
            for _ in 0..20 {
                current = current.mutate(&mut rng, 0.2);
            }
            current
        };

        assert_eq!(run(), run());
    }
}
