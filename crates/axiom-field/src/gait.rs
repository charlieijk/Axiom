//! A fixed open-loop reference gait.
//!
//! This is deliberately not evolved and not clever. It is a known input, so
//! that the simulator can be compared against itself over time and — once
//! hardware exists — against the same commands running on real servos. A
//! reference input is the only way to tell a modelling error apart from a
//! transfer gap.

use crate::sim::{ACTUATOR_COUNT, LEG_COUNT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    /// Gait cycles per second.
    pub frequency_hz: f32,
    /// Hip swing amplitude as a fraction of servo travel.
    pub hip_amplitude: f32,
    /// Knee lift amplitude as a fraction of servo travel.
    pub knee_amplitude: f32,
    /// Knee phase lead over its hip, in cycles. This sets travel direction.
    pub knee_phase_offset: f32,
}

impl Default for Gait {
    fn default() -> Self {
        // A diagonal trot: each leg leads the one diagonally opposite by half
        // a cycle, and each knee runs in antiphase with its own hip.
        //
        // These numbers were measured with `axiom-field gait`, not derived,
        // and two of them are counter-intuitive. The knee offset sets travel
        // direction: at +/-0.25 this same robot walks backwards, and only
        // antiphase drives it forward. And amplitude is not throttle — a
        // bigger stride topples the robot rather than moving it further.
        // Hip 0.45 / knee 0.35 ends upside down (tilt 3.14 rad) while the
        // smaller stride below walks 0.94 m in 10 s, dead straight.
        Self {
            frequency_hz: 2.0,
            hip_amplitude: 0.15,
            knee_amplitude: 0.12,
            knee_phase_offset: 0.5,
        }
    }
}

impl Gait {
    /// Actions for one control tick, in actuator order.
    pub fn actions_at(&self, time_s: f32) -> Vec<f32> {
        let mut actions = vec![0.0; ACTUATOR_COUNT];
        for leg in 0..LEG_COUNT {
            // Legs 0 and 3 (front-right, rear-left) move together, opposite
            // legs 1 and 2 — the diagonal pairs of a trot.
            let diagonal_phase = if leg == 0 || leg == 3 { 0.0 } else { 0.5 };
            let cycles = time_s * self.frequency_hz + diagonal_phase;

            actions[leg * 2] = self.hip_amplitude * sine_cycles(cycles);
            actions[leg * 2 + 1] =
                self.knee_amplitude * sine_cycles(cycles + self.knee_phase_offset);
        }
        actions
    }
}

fn sine_cycles(cycles: f32) -> f32 {
    (cycles * std::f32::consts::TAU).sin()
}

/// Convenience for the default trot.
pub fn sine_gait() -> Gait {
    Gait::default()
}

#[cfg(test)]
mod tests {
    use super::{Gait, sine_gait};
    use crate::sim::ACTUATOR_COUNT;

    #[test]
    fn a_gait_commands_every_actuator() {
        assert_eq!(sine_gait().actions_at(0.4).len(), ACTUATOR_COUNT);
    }

    #[test]
    fn every_command_stays_inside_the_unit_range() {
        let gait = sine_gait();
        for tick in 0..400 {
            for action in gait.actions_at(tick as f32 * 0.05) {
                assert!((-1.0..=1.0).contains(&action), "{action} left the range");
            }
        }
    }

    #[test]
    fn diagonal_legs_move_together_and_oppose_the_other_pair() {
        let gait = Gait {
            frequency_hz: 1.0,
            knee_phase_offset: 0.0,
            ..Gait::default()
        };
        let actions = gait.actions_at(0.125);

        // Front-right (leg 0) and rear-left (leg 3) share a phase.
        assert!((actions[0] - actions[6]).abs() < 1e-6);
        // Front-left (leg 1) and rear-right (leg 2) share the opposite phase.
        assert!((actions[2] - actions[4]).abs() < 1e-6);
        assert!((actions[0] + actions[2]).abs() < 1e-6);
    }

    #[test]
    fn the_gait_is_periodic() {
        let gait = sine_gait();
        let period = 1.0 / gait.frequency_hz;

        let first = gait.actions_at(0.3);
        let second = gait.actions_at(0.3 + period);

        for (a, b) in first.iter().zip(second.iter()) {
            assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn amplitudes_scale_the_commands_they_own() {
        let quiet = Gait {
            hip_amplitude: 0.1,
            ..Gait::default()
        };
        let loud = Gait {
            hip_amplitude: 0.5,
            ..Gait::default()
        };

        let peak = |gait: Gait| {
            (0..80)
                .map(|tick| gait.actions_at(tick as f32 * 0.01)[0].abs())
                .fold(0.0_f32, f32::max)
        };

        assert!(peak(loud) > peak(quiet) * 4.0);
    }
}
