//! Recorded runs, and the rules for comparing them.
//!
//! Two different claims live here, and conflating them would make the gate
//! dishonest:
//!
//! * **Within one binary**, the same inputs produce bit-identical output. That
//!   is asserted exactly, with no tolerance, and it is a strong claim: it is
//!   what makes an evolutionary search reproducible.
//! * **Across architectures**, they do not. Legged contact is close to
//!   chaotic, so the floating-point differences between x86_64 and aarch64
//!   amplify rather than average out: the same commit diverges by ~10 mm
//!   within two seconds of walking. No fixed pointwise band is both meaningful
//!   and passing, so [`Trajectory::compare`] is a same-machine tool and the
//!   committed golden is compared as a behavioural [`TrajectorySummary`].
//!
//! The honest consequence is that the committed artifact anchors *behaviour*
//! — it walks, forward, upright, roughly this far — and cannot detect a small
//! parameter change. Detecting those needs two runs from one binary, which is
//! what `a_small_parameter_change_is_detected_within_one_binary` does.

use serde::{Deserialize, Serialize};

use crate::{config::RobotConfig, gait::Gait, sim::FieldSim, sim::Sample};

/// Positional agreement required of a replayed golden trajectory, in metres.
/// One millimetre over a run that travels centimetres: solver noise passes,
/// a changed model does not.
pub const POSITION_TOLERANCE_M: f32 = 1.0e-3;
/// Angular agreement required, in radians (roughly 0.6 degrees).
pub const ANGLE_TOLERANCE_RAD: f32 = 1.0e-2;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Trajectory {
    /// The parameter set this run was recorded against.
    pub robot: String,
    /// Whether those parameters were measured off hardware. A golden recorded
    /// against uncalibrated parameters describes a robot nobody has built.
    pub calibrated: bool,
    pub control_hz: f32,
    pub ticks: usize,
    /// Only every `stride`th sample is stored, to keep the file reviewable.
    pub stride: usize,
    pub samples: Vec<Sample>,
}

/// The coarse behavioural shape of a run: what a comparison can rely on when
/// the two runs did not come from the same machine.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrajectorySummary {
    pub forward_travel_m: f32,
    pub lateral_drift_m: f32,
    pub final_height_m: f32,
    pub max_tilt_rad: f32,
}

#[derive(Debug)]
pub struct Divergence {
    pub tick: usize,
    pub field: &'static str,
    pub expected: f32,
    pub actual: f32,
    pub tolerance: f32,
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "tick {}: {} expected {:.6} but got {:.6} (tolerance {:.6}, off by {:.6})",
            self.tick,
            self.field,
            self.expected,
            self.actual,
            self.tolerance,
            (self.expected - self.actual).abs()
        )
    }
}

/// Ticks of unactuated settling before a recording starts, so that travel
/// measures walking rather than the robot dropping the last few millimetres
/// onto its feet.
pub const SETTLE_TICKS: usize = 100;

impl Trajectory {
    /// Settles the robot, then runs the reference gait and records the result.
    pub fn record(config: RobotConfig, gait: Gait, ticks: usize, stride: usize) -> Self {
        let stride = stride.max(1);
        let control_hz = config.sim.control_hz;
        let robot = config.meta.name.clone();
        let calibrated = config.meta.calibrated;
        let mut sim = FieldSim::new(config);
        sim.settle(SETTLE_TICKS);
        let mut samples = Vec::new();

        for tick in 0..ticks {
            let time_s = tick as f32 / control_hz;
            let sample = sim.control_step(&gait.actions_at(time_s));
            if tick % stride == 0 || tick + 1 == ticks {
                samples.push(sample);
            }
        }

        Self {
            robot,
            calibrated,
            control_hz,
            ticks,
            stride,
            samples,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("trajectory serialization cannot fail")
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Net forward travel over the whole run.
    pub fn forward_travel_m(&self) -> f32 {
        match (self.samples.first(), self.samples.last()) {
            (Some(first), Some(last)) => last.forward_m() - first.forward_m(),
            _ => 0.0,
        }
    }

    /// The coarse behavioural shape of a run.
    ///
    /// This is what survives a change of architecture. Pointwise position does
    /// not: contact dynamics amplify floating-point differences, so the same
    /// build on x86_64 and aarch64 drifts millimetres apart within a couple of
    /// seconds and further after that.
    pub fn summary(&self) -> TrajectorySummary {
        let (Some(first), Some(last)) = (self.samples.first(), self.samples.last()) else {
            return TrajectorySummary::default();
        };
        TrajectorySummary {
            forward_travel_m: last.forward_m() - first.forward_m(),
            lateral_drift_m: last.lateral_m() - first.lateral_m(),
            final_height_m: last.height_m(),
            max_tilt_rad: self
                .samples
                .iter()
                .map(|sample| sample.tilt_rad)
                .fold(0.0_f32, f32::max),
        }
    }

    /// Compares against a reference, reporting the first sample that drifts
    /// outside the tolerance band rather than a bare boolean.
    ///
    /// **Same machine only.** Use this to compare two runs from one binary, or
    /// a recording against a replay on the hardware that produced it. Across
    /// architectures, compare [`Trajectory::summary`] instead.
    pub fn compare(&self, reference: &Self) -> Result<(), Divergence> {
        if self.samples.len() != reference.samples.len() {
            return Err(Divergence {
                tick: 0,
                field: "sample count",
                expected: reference.samples.len() as f32,
                actual: self.samples.len() as f32,
                tolerance: 0.0,
            });
        }

        for (actual, expected) in self.samples.iter().zip(reference.samples.iter()) {
            let positions = [
                ("position x", actual.position_m[0], expected.position_m[0]),
                ("position y", actual.position_m[1], expected.position_m[1]),
                ("position z", actual.position_m[2], expected.position_m[2]),
            ];
            for (field, actual_value, expected_value) in positions {
                if (actual_value - expected_value).abs() > POSITION_TOLERANCE_M {
                    return Err(Divergence {
                        tick: expected.tick,
                        field,
                        expected: expected_value,
                        actual: actual_value,
                        tolerance: POSITION_TOLERANCE_M,
                    });
                }
            }

            let angles = [
                ("tilt", actual.tilt_rad, expected.tilt_rad),
                ("pitch", actual.pitch_rad, expected.pitch_rad),
                ("heading", actual.heading_rad, expected.heading_rad),
            ];
            for (field, actual_angle, expected_angle) in angles {
                if (actual_angle - expected_angle).abs() > ANGLE_TOLERANCE_RAD {
                    return Err(Divergence {
                        tick: expected.tick,
                        field,
                        expected: expected_angle,
                        actual: actual_angle,
                        tolerance: ANGLE_TOLERANCE_RAD,
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Trajectory;
    use crate::{config::RobotConfig, gait::sine_gait};

    #[test]
    fn a_recording_round_trips_through_json() {
        let trajectory = Trajectory::record(RobotConfig::nominal(), sine_gait(), 20, 4);
        let decoded = Trajectory::from_json(&trajectory.to_json()).expect("valid json");

        assert_eq!(decoded, trajectory);
    }

    #[test]
    fn a_recording_matches_itself_rerun() {
        let first = Trajectory::record(RobotConfig::nominal(), sine_gait(), 40, 4);
        let second = Trajectory::record(RobotConfig::nominal(), sine_gait(), 40, 4);

        // Same binary, same inputs: exact equality, no tolerance.
        assert_eq!(first, second);
        first.compare(&second).expect("identical runs must compare");
    }

    #[test]
    fn a_changed_parameter_breaks_the_comparison() {
        // The tolerance band must not be so wide that it hides a real change.
        let reference = Trajectory::record(RobotConfig::nominal(), sine_gait(), 60, 4);

        let mut heavier = RobotConfig::nominal();
        heavier.body.mass_kg *= 2.0;
        let changed = Trajectory::record(heavier, sine_gait(), 60, 4);

        let divergence = changed
            .compare(&reference)
            .expect_err("doubling chassis mass must be detected");
        assert!(divergence.to_string().contains("tick"));
    }

    #[test]
    fn a_divergence_names_the_field_and_the_tick() {
        let reference = Trajectory::record(RobotConfig::nominal(), sine_gait(), 30, 3);
        let mut shifted = reference.clone();
        shifted.samples[2].position_m[0] += 1.0;

        let divergence = shifted
            .compare(&reference)
            .expect_err("shift is detectable");

        assert_eq!(divergence.field, "position x");
        assert_eq!(divergence.tick, reference.samples[2].tick);
    }
}
