//! Robot parameters, loaded from `robot.toml`.
//!
//! Nothing in the simulator hardcodes a physical constant. Calibrating against
//! a real robot is meant to be a diff of the parameter file, not a code change.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Meta {
    pub name: String,
    /// False until every value came off physical hardware.
    pub calibrated: bool,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BodyConfig {
    pub length_m: f32,
    pub width_m: f32,
    pub height_m: f32,
    pub mass_kg: f32,
    pub hip_inset: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LegConfig {
    pub upper_length_m: f32,
    pub upper_mass_kg: f32,
    pub lower_length_m: f32,
    pub lower_mass_kg: f32,
    pub link_radius_m: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StanceConfig {
    pub hip_deg: f32,
    pub knee_deg: f32,
}

impl StanceConfig {
    pub fn hip_rad(&self) -> f32 {
        self.hip_deg.to_radians()
    }

    pub fn knee_rad(&self) -> f32 {
        self.knee_deg.to_radians()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ServoConfig {
    pub max_rate_deg_per_s: f32,
    pub min_angle_deg: f32,
    pub max_angle_deg: f32,
    pub max_torque_nm: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl ServoConfig {
    pub fn min_angle_rad(&self) -> f32 {
        self.min_angle_deg.to_radians()
    }

    pub fn max_angle_rad(&self) -> f32 {
        self.max_angle_deg.to_radians()
    }

    pub fn max_rate_rad_per_s(&self) -> f32 {
        self.max_rate_deg_per_s.to_radians()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContactConfig {
    pub friction: f32,
    pub restitution: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SimConfig {
    pub control_hz: f32,
    pub physics_substeps: usize,
    pub gravity_m_s2: f32,
    pub spawn_clearance_m: f32,
}

impl SimConfig {
    /// Seconds per control tick — the rate the firmware will run at.
    pub fn control_dt(&self) -> f32 {
        1.0 / self.control_hz
    }

    /// Seconds per physics step.
    pub fn physics_dt(&self) -> f32 {
        self.control_dt() / self.physics_substeps.max(1) as f32
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RobotConfig {
    pub meta: Meta,
    pub body: BodyConfig,
    pub leg: LegConfig,
    pub stance: StanceConfig,
    pub servo: ServoConfig,
    pub contact: ContactConfig,
    pub sim: SimConfig,
}

impl RobotConfig {
    /// Chassis-centre height at which the crouched feet just touch the floor.
    ///
    /// Derived from the stance and the link geometry — including the capsule
    /// cap at the foot, which is a real 8 mm and easy to forget — so that
    /// changing a leg length cannot silently spawn the robot inside the floor.
    pub fn standing_height_m(&self) -> f32 {
        let hip = self.stance.hip_rad();
        let knee = self.stance.knee_rad();
        self.body.height_m * 0.5
            + self.leg.upper_length_m * hip.cos()
            + self.leg.lower_length_m * (hip + knee).cos()
            + self.leg.link_radius_m
    }

    /// Where the chassis centre starts: standing height plus the drop.
    pub fn spawn_height_m(&self) -> f32 {
        self.standing_height_m() + self.sim.spawn_clearance_m
    }
}

/// The parameter file shipped with the crate, used when no path is given.
pub const DEFAULT_ROBOT_TOML: &str = include_str!("../robot.toml");

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "reading robot parameters: {error}"),
            Self::Parse(error) => write!(formatter, "parsing robot parameters: {error}"),
            Self::Invalid(reason) => write!(formatter, "invalid robot parameters: {reason}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl RobotConfig {
    pub fn nominal() -> Self {
        Self::from_toml(DEFAULT_ROBOT_TOML).expect("bundled robot.toml must be valid")
    }

    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml(&text)
    }

    /// Rejects parameters that would produce a robot the physics engine cannot
    /// represent, so a typo in the file fails loudly instead of quietly
    /// producing a plausible-looking but meaningless run.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut problems = Vec::new();
        let positive = [
            ("body.length_m", self.body.length_m),
            ("body.width_m", self.body.width_m),
            ("body.height_m", self.body.height_m),
            ("body.mass_kg", self.body.mass_kg),
            ("leg.upper_length_m", self.leg.upper_length_m),
            ("leg.upper_mass_kg", self.leg.upper_mass_kg),
            ("leg.lower_length_m", self.leg.lower_length_m),
            ("leg.lower_mass_kg", self.leg.lower_mass_kg),
            ("leg.link_radius_m", self.leg.link_radius_m),
            ("servo.max_rate_deg_per_s", self.servo.max_rate_deg_per_s),
            ("servo.max_torque_nm", self.servo.max_torque_nm),
            ("sim.control_hz", self.sim.control_hz),
            ("sim.gravity_m_s2", self.sim.gravity_m_s2),
        ];
        for (name, value) in positive {
            // NaN must fail too, which a bare `<= 0.0` would let through.
            if value.is_nan() || value <= 0.0 {
                problems.push(format!("{name} must be greater than zero"));
            }
        }

        if self.servo.max_angle_deg <= self.servo.min_angle_deg {
            problems.push("servo.max_angle_deg must exceed servo.min_angle_deg".to_string());
        }
        if self.sim.physics_substeps == 0 {
            problems.push("sim.physics_substeps must be at least 1".to_string());
        }
        if self.body.hip_inset.is_nan() || self.body.hip_inset <= 0.0 || self.body.hip_inset > 1.0 {
            problems.push("body.hip_inset must fall in (0, 1]".to_string());
        }
        if self.contact.friction.is_nan() || self.contact.friction < 0.0 {
            problems.push("contact.friction must not be negative".to_string());
        }
        // A leg that cannot reach the floor makes every gait result meaningless.
        let reach = self.leg.upper_length_m + self.leg.lower_length_m;
        if reach <= self.body.height_m * 0.5 {
            problems.push(format!(
                "leg reach {reach:.3} m cannot clear the chassis; the robot would rest on its belly"
            ));
        }
        if self.sim.spawn_clearance_m < 0.0 {
            problems.push("sim.spawn_clearance_m must not be negative".to_string());
        }
        // The stance must actually hold the robot off the floor. A stance that
        // folds the leg flat leaves the chassis resting on its belly, and every
        // downstream locomotion number becomes meaningless.
        if self.standing_height_m() <= self.body.height_m * 0.5 + self.leg.link_radius_m {
            problems.push(format!(
                "stance hip {:.1} deg / knee {:.1} deg collapses the leg; the chassis would rest on the floor",
                self.stance.hip_deg, self.stance.knee_deg
            ));
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(problems.join("; ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RobotConfig, SimConfig};

    #[test]
    fn bundled_parameters_load_and_validate() {
        let config = RobotConfig::nominal();

        assert_eq!(config.meta.name, "nominal-mg90s-quadruped");
        // The bundled file describes a robot nobody has built. If this ever
        // flips, the golden trajectories were recorded against a different
        // machine and must be re-recorded.
        assert!(!config.meta.calibrated);
    }

    #[test]
    fn control_and_physics_rates_agree() {
        let sim = SimConfig {
            control_hz: 20.0,
            physics_substeps: 5,
            gravity_m_s2: 9.81,
            spawn_clearance_m: 0.005,
        };

        assert!((sim.control_dt() - 0.05).abs() < 1e-6);
        assert!((sim.physics_dt() - 0.01).abs() < 1e-6);
    }

    #[test]
    fn invalid_parameters_are_rejected_with_every_reason() {
        let mut config = RobotConfig::nominal();
        config.body.mass_kg = 0.0;
        config.servo.max_angle_deg = config.servo.min_angle_deg;

        let error = config.validate().expect_err("invalid config must fail");
        let message = error.to_string();

        assert!(message.contains("body.mass_kg"), "{message}");
        assert!(message.contains("servo.max_angle_deg"), "{message}");
    }

    #[test]
    fn a_leg_too_short_to_reach_the_floor_is_rejected() {
        let mut config = RobotConfig::nominal();
        config.leg.upper_length_m = 0.005;
        config.leg.lower_length_m = 0.005;

        let error = config.validate().expect_err("unreachable floor must fail");
        assert!(error.to_string().contains("rest on its belly"));
    }

    #[test]
    fn servo_angles_convert_to_radians() {
        let config = RobotConfig::nominal();

        assert!((config.servo.min_angle_rad() - (-75.0_f32).to_radians()).abs() < 1e-6);
        assert!((config.servo.max_rate_rad_per_s() - 300.0_f32.to_radians()).abs() < 1e-6);
    }
}
