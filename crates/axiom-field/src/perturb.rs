//! Domain randomization: the ways a real robot differs from its parameters.
//!
//! Every value in `robot.toml` is a guess, and the ones that matter most are
//! the ones hardest to measure — the friction of a particular floor, the
//! loaded speed of a particular servo, how square a servo horn was pressed
//! onto its spline. A controller tuned to one exact parameter set is tuned to
//! a robot that does not exist.
//!
//! So a genome is scored across an ensemble of perturbed worlds rather than
//! one nominal world. That is the standard defence against a simulator's own
//! idiosyncrasies, and it is the closest a software-only stage can get to
//! testing transfer.

use axiom::rng::Rng;

use crate::{config::RobotConfig, sim::ACTUATOR_COUNT};

/// How far each quantity may stray from nominal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    /// Fractional spread on chassis mass, e.g. `0.15` for +/-15%.
    pub mass: f32,
    pub friction: f32,
    pub servo_rate: f32,
    pub servo_torque: f32,
    /// Absolute per-joint mounting error, in radians.
    pub joint_offset_rad: f32,
}

impl Default for Spread {
    fn default() -> Self {
        // Mass is the best-known of these; floor friction is the worst, since
        // it depends on a surface nobody has chosen yet. The joint offset is
        // three degrees, about one tooth on a 25-tooth servo spline.
        Self {
            mass: 0.15,
            friction: 0.30,
            servo_rate: 0.20,
            servo_torque: 0.20,
            joint_offset_rad: 3.0_f32 * std::f32::consts::PI / 180.0,
        }
    }
}

impl Spread {
    /// A spread that produces only the nominal world.
    pub fn none() -> Self {
        Self {
            mass: 0.0,
            friction: 0.0,
            servo_rate: 0.0,
            servo_torque: 0.0,
            joint_offset_rad: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perturbation {
    pub mass_scale: f32,
    pub friction_scale: f32,
    pub servo_rate_scale: f32,
    pub servo_torque_scale: f32,
    /// Mounting error per joint: the angle the link actually holds when the
    /// servo believes it is at its commanded position.
    pub joint_offsets_rad: [f32; ACTUATOR_COUNT],
}

impl Default for Perturbation {
    fn default() -> Self {
        Self::nominal()
    }
}

impl Perturbation {
    /// The unperturbed world, exactly as `robot.toml` describes it.
    pub fn nominal() -> Self {
        Self {
            mass_scale: 1.0,
            friction_scale: 1.0,
            servo_rate_scale: 1.0,
            servo_torque_scale: 1.0,
            joint_offsets_rad: [0.0; ACTUATOR_COUNT],
        }
    }

    pub fn sample(rng: &mut Rng, spread: &Spread) -> Self {
        let mut joint_offsets_rad = [0.0; ACTUATOR_COUNT];
        for offset in &mut joint_offsets_rad {
            *offset = rng.range_f32(-spread.joint_offset_rad, spread.joint_offset_rad);
        }
        Self {
            mass_scale: 1.0 + rng.range_f32(-spread.mass, spread.mass),
            friction_scale: 1.0 + rng.range_f32(-spread.friction, spread.friction),
            servo_rate_scale: 1.0 + rng.range_f32(-spread.servo_rate, spread.servo_rate),
            servo_torque_scale: 1.0 + rng.range_f32(-spread.servo_torque, spread.servo_torque),
            joint_offsets_rad,
        }
    }

    /// An ensemble of worlds drawn from one seed.
    pub fn ensemble(seed: u64, spread: &Spread, count: usize) -> Vec<Self> {
        let mut rng = Rng::new(seed);
        (0..count).map(|_| Self::sample(&mut rng, spread)).collect()
    }

    /// Applies the scalar factors, leaving the joint offsets for the simulator.
    ///
    /// Geometry is deliberately untouched: link lengths are the one thing a
    /// builder can measure accurately with calipers, so randomizing them would
    /// model an uncertainty that does not exist.
    pub fn apply(&self, config: &RobotConfig) -> RobotConfig {
        let mut config = config.clone();
        config.body.mass_kg *= self.mass_scale;
        config.contact.friction *= self.friction_scale;
        config.servo.max_rate_deg_per_s *= self.servo_rate_scale;
        config.servo.max_torque_nm *= self.servo_torque_scale;
        config
    }

    pub fn is_nominal(&self) -> bool {
        *self == Self::nominal()
    }
}

#[cfg(test)]
mod tests {
    use super::{Perturbation, Spread};
    use crate::config::RobotConfig;
    use axiom::rng::Rng;

    #[test]
    fn the_nominal_world_changes_nothing() {
        let config = RobotConfig::nominal();
        let applied = Perturbation::nominal().apply(&config);

        assert_eq!(applied, config);
        assert!(Perturbation::nominal().is_nominal());
    }

    #[test]
    fn a_zero_spread_samples_only_the_nominal_world() {
        let mut rng = Rng::new(7);
        let sampled = Perturbation::sample(&mut rng, &Spread::none());

        assert!(sampled.is_nominal(), "{sampled:?}");
    }

    #[test]
    fn samples_stay_inside_their_spread() {
        let spread = Spread::default();
        let mut rng = Rng::new(11);

        for _ in 0..200 {
            let sample = Perturbation::sample(&mut rng, &spread);
            assert!((sample.mass_scale - 1.0).abs() <= spread.mass + 1e-6);
            assert!((sample.friction_scale - 1.0).abs() <= spread.friction + 1e-6);
            assert!((sample.servo_rate_scale - 1.0).abs() <= spread.servo_rate + 1e-6);
            assert!((sample.servo_torque_scale - 1.0).abs() <= spread.servo_torque + 1e-6);
            for offset in sample.joint_offsets_rad {
                assert!(offset.abs() <= spread.joint_offset_rad + 1e-6);
            }
        }
    }

    #[test]
    fn an_ensemble_is_reproducible_from_its_seed_and_varied_within_itself() {
        let spread = Spread::default();
        let first = Perturbation::ensemble(42, &spread, 6);
        let second = Perturbation::ensemble(42, &spread, 6);
        let other = Perturbation::ensemble(43, &spread, 6);

        assert_eq!(first, second);
        assert_ne!(first, other);
        // The worlds inside one ensemble must actually differ from each other,
        // or scoring across it proves nothing.
        assert_ne!(first[0], first[1]);
    }

    #[test]
    fn a_perturbed_config_still_validates() {
        // Randomization must not produce a robot the loader would reject; a
        // silently invalid world would poison every score drawn from it.
        let config = RobotConfig::nominal();
        let spread = Spread::default();
        let mut rng = Rng::new(3);

        for _ in 0..100 {
            let perturbed = Perturbation::sample(&mut rng, &spread).apply(&config);
            perturbed
                .validate()
                .expect("a randomized world must remain valid");
        }
    }

    #[test]
    fn geometry_is_never_randomized() {
        let config = RobotConfig::nominal();
        let mut rng = Rng::new(5);
        let perturbed = Perturbation::sample(&mut rng, &Spread::default()).apply(&config);

        assert_eq!(perturbed.leg, config.leg);
        assert_eq!(perturbed.body.length_m, config.body.length_m);
        assert_eq!(perturbed.stance, config.stance);
        // Standing height is derived from geometry, so it must be unchanged.
        assert_eq!(perturbed.standing_height_m(), config.standing_height_m());
    }
}
