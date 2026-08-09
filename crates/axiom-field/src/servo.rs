//! A hobby servo, modelled as what it actually is.
//!
//! Axiom's original simulator treated an action as a thrust term. A real hobby
//! servo is nothing like that: it accepts a *position* command, drives toward
//! it with an internal loop, cannot slew faster than its rated speed, and
//! cannot exceed its stall torque. Those three limits are what make a
//! simulated gait resemble a real one, so they are modelled explicitly here
//! and the torque cap is handed to the physics engine as a motor force limit.

use crate::config::ServoConfig;

#[derive(Clone, Debug)]
pub struct Servo {
    min_rad: f32,
    max_rad: f32,
    max_rate_rad_per_s: f32,
    /// The angle a zero action holds — the joint's neutral stance, not
    /// necessarily the middle of the mechanical travel.
    centre_rad: f32,
    /// The angle the servo has actually been told to hold, after rate limiting.
    commanded_rad: f32,
}

impl Servo {
    /// Starts the servo at the centre of its mechanical travel.
    pub fn new(config: &ServoConfig) -> Self {
        let centre = 0.5 * (config.min_angle_rad() + config.max_angle_rad());
        Self::with_centre(config, centre)
    }

    /// Starts the servo holding `centre_rad`, the joint's neutral stance.
    ///
    /// A zero action means "stand", not "point straight down", so a controller
    /// that does nothing leaves the robot in a pose it can actually hold.
    pub fn with_centre(config: &ServoConfig, centre_rad: f32) -> Self {
        let min_rad = config.min_angle_rad();
        let max_rad = config.max_angle_rad();
        let centre_rad = centre_rad.clamp(min_rad, max_rad);
        Self {
            min_rad,
            max_rad,
            max_rate_rad_per_s: config.max_rate_rad_per_s(),
            centre_rad,
            commanded_rad: centre_rad,
        }
    }

    /// Maps a normalized action in `[-1, 1]` onto travel around the stance.
    ///
    /// An off-centre stance means one direction reaches its mechanical stop
    /// before the other; the target is clamped rather than rescaled, so the
    /// asymmetry a real robot has is preserved instead of hidden.
    pub fn target_for(&self, action: f32) -> f32 {
        let action = action.clamp(-1.0, 1.0);
        let half_span = 0.5 * (self.max_rad - self.min_rad);
        (self.centre_rad + action * half_span).clamp(self.min_rad, self.max_rad)
    }

    pub fn centre_rad(&self) -> f32 {
        self.centre_rad
    }

    /// Advances the commanded angle toward `action` by at most one tick's slew.
    ///
    /// Returns the new commanded angle. A controller that demands a step
    /// change gets the ramp a real servo would give it, not teleportation.
    pub fn step(&mut self, action: f32, dt: f32) -> f32 {
        let target = self.target_for(action);
        let max_step = (self.max_rate_rad_per_s * dt).max(0.0);
        let delta = (target - self.commanded_rad).clamp(-max_step, max_step);
        self.commanded_rad = (self.commanded_rad + delta).clamp(self.min_rad, self.max_rad);
        self.commanded_rad
    }

    pub fn commanded_rad(&self) -> f32 {
        self.commanded_rad
    }

    /// True when the servo is still slewing toward its commanded target.
    pub fn is_saturated(&self, action: f32, dt: f32) -> bool {
        let target = self.target_for(action);
        (target - self.commanded_rad).abs() > self.max_rate_rad_per_s * dt
    }
}

#[cfg(test)]
mod tests {
    use super::Servo;
    use crate::config::RobotConfig;

    fn servo() -> Servo {
        Servo::new(&RobotConfig::nominal().servo)
    }

    #[test]
    fn a_new_servo_parks_at_the_centre_of_its_travel() {
        // The nominal file is symmetric, so centre is zero.
        assert!(servo().commanded_rad().abs() < 1e-6);
    }

    #[test]
    fn a_stance_servo_parks_on_its_stance_and_holds_it_for_a_zero_action() {
        let config = RobotConfig::nominal();
        let stance = config.stance.hip_rad();
        let servo = Servo::with_centre(&config.servo, stance);

        assert!((servo.commanded_rad() - stance).abs() < 1e-6);
        assert!((servo.target_for(0.0) - stance).abs() < 1e-6);
    }

    #[test]
    fn an_off_centre_stance_still_respects_the_mechanical_stops() {
        let config = RobotConfig::nominal();
        let servo = Servo::with_centre(&config.servo, config.stance.hip_rad());

        assert!(servo.target_for(1.0) <= config.servo.max_angle_rad() + 1e-6);
        assert!(servo.target_for(-1.0) >= config.servo.min_angle_rad() - 1e-6);
    }

    #[test]
    fn actions_map_onto_the_full_travel() {
        let servo = servo();
        let config = RobotConfig::nominal();

        assert!((servo.target_for(-1.0) - config.servo.min_angle_rad()).abs() < 1e-6);
        assert!((servo.target_for(1.0) - config.servo.max_angle_rad()).abs() < 1e-6);
        assert!(servo.target_for(0.0).abs() < 1e-6);
    }

    #[test]
    fn actions_beyond_the_unit_range_are_clamped_not_extrapolated() {
        let servo = servo();
        let config = RobotConfig::nominal();

        assert!((servo.target_for(9.0) - config.servo.max_angle_rad()).abs() < 1e-6);
        assert!((servo.target_for(-9.0) - config.servo.min_angle_rad()).abs() < 1e-6);
    }

    #[test]
    fn a_step_command_is_rate_limited_instead_of_teleporting() {
        let config = RobotConfig::nominal();
        let mut servo = Servo::new(&config.servo);
        let dt = config.sim.control_dt();
        let one_tick = config.servo.max_rate_rad_per_s() * dt;

        // Demand the far end of travel in a single tick.
        let after_one = servo.step(1.0, dt);

        assert!(
            (after_one - one_tick).abs() < 1e-6,
            "expected one tick of slew ({one_tick}), got {after_one}"
        );
        assert!(after_one < config.servo.max_angle_rad());
    }

    #[test]
    fn a_held_command_eventually_reaches_and_stops_at_the_target() {
        let config = RobotConfig::nominal();
        let mut servo = Servo::new(&config.servo);
        let dt = config.sim.control_dt();

        for _ in 0..200 {
            servo.step(1.0, dt);
        }

        assert!((servo.commanded_rad() - config.servo.max_angle_rad()).abs() < 1e-6);
        assert!(!servo.is_saturated(1.0, dt));
    }

    #[test]
    fn the_commanded_angle_never_leaves_the_mechanical_limits() {
        let config = RobotConfig::nominal();
        let mut servo = Servo::new(&config.servo);
        let dt = config.sim.control_dt();

        for tick in 0..500 {
            // Alternate hard against both stops.
            servo.step(if tick % 2 == 0 { 5.0 } else { -5.0 }, dt);
            assert!(servo.commanded_rad() >= config.servo.min_angle_rad() - 1e-6);
            assert!(servo.commanded_rad() <= config.servo.max_angle_rad() + 1e-6);
        }
    }
}
