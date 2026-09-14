//! A rigid-body quadruped and the fixed-tick loop that drives it.
//!
//! The morphology is fixed and buildable: one chassis, four legs, two links
//! per leg, and one servo per joint. Unlike Axiom's abstract sandbox, forward
//! motion here is not a term in an equation — it can only come from joint
//! torque acting through foot contact, which is what makes a result here worth
//! comparing against hardware.
//!
//! Frame convention: `x` forward, `y` up, `z` to the robot's right.

use rapier3d::prelude::*;

use crate::{config::RobotConfig, servo::Servo};

/// Legs in a fixed order: front-right, front-left, rear-right, rear-left.
/// Actuators follow the same order, hip before knee, so action index `2 * leg`
/// is a hip and `2 * leg + 1` is its knee.
pub const LEG_COUNT: usize = 4;
pub const ACTUATOR_COUNT: usize = LEG_COUNT * 2;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Sample {
    pub tick: usize,
    pub time_s: f32,
    /// Chassis centre in world metres.
    pub position_m: [f32; 3],
    /// Angle between the chassis "up" and world up. Always non-negative: this
    /// is how far the robot has tipped, in any direction.
    pub tilt_rad: f32,
    /// Nose elevation above horizontal, positive nose-up.
    pub pitch_rad: f32,
    /// Ground-plane heading of the nose. Drift here is how a real quadruped
    /// walks in a curve instead of a straight line.
    pub heading_rad: f32,
    /// Measured joint angles, in actuator order.
    pub joint_rad: Vec<f32>,
}

impl Sample {
    pub fn forward_m(&self) -> f32 {
        self.position_m[0]
    }

    pub fn height_m(&self) -> f32 {
        self.position_m[1]
    }

    /// Lateral drift, which a real quadruped produces plenty of.
    pub fn lateral_m(&self) -> f32 {
        self.position_m[2]
    }
}

/// The signed rotation about `z` encoded in a quaternion (its twist).
///
/// Both joints hinge about `z`, so this reads back the hinge angle without
/// going through an Euler decomposition and its ambiguities.
fn twist_about_z(rotation: Rotation) -> f32 {
    // A quaternion and its negation are the same rotation. Picking the
    // positive-w hemisphere keeps the result in [-pi, pi], so a joint reading
    // never jumps by a full turn and can be compared against its limits.
    let (z, w) = if rotation.w < 0.0 {
        (-rotation.z, -rotation.w)
    } else {
        (rotation.z, rotation.w)
    };
    2.0 * z.atan2(w)
}

struct Actuator {
    joint: ImpulseJointHandle,
    parent: RigidBodyHandle,
    child: RigidBodyHandle,
    servo: Servo,
    /// Mounting error: the servo believes it holds its commanded angle, but
    /// the link actually sits this far away from it.
    calibration_offset_rad: f32,
}

/// A renderable collider pose copied directly from the rigid-body world.
#[derive(Clone, Debug, serde::Serialize)]
pub struct BodyPose {
    pub kind: &'static str,
    pub shape: &'static str,
    pub size: [f32; 3],
    pub position: [f32; 3],
    /// Quaternion in browser / Three.js order: x, y, z, w.
    pub rotation: [f32; 4],
}

pub struct FieldSim {
    config: RobotConfig,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    ccd_solver: CCDSolver,
    integration_parameters: IntegrationParameters,
    gravity: Vector,
    chassis: RigidBodyHandle,
    actuators: Vec<Actuator>,
    tick: usize,
}

impl FieldSim {
    pub fn new(config: RobotConfig) -> Self {
        Self::with_calibration(config, [0.0; ACTUATOR_COUNT])
    }

    /// Builds the robot with per-joint mounting errors.
    ///
    /// The links are still assembled at the nominal stance and only the motor
    /// targets carry the offset, which is what happens physically: a robot
    /// with a mis-pressed servo horn twitches into its real pose on power-up.
    /// Building the geometry offset instead would risk spawning a foot below
    /// the floor, since spawn height is derived from the nominal stance.
    pub fn with_calibration(
        config: RobotConfig,
        calibration_offsets_rad: [f32; ACTUATOR_COUNT],
    ) -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut impulse_joints = ImpulseJointSet::new();

        let half_length = config.body.length_m * 0.5;
        let half_width = config.body.width_m * 0.5;
        let half_height = config.body.height_m * 0.5;
        let radius = config.leg.link_radius_m;

        // Floor. Thick enough that nothing tunnels through it at these speeds.
        let ground =
            bodies.insert(RigidBodyBuilder::fixed().translation(Vec3::new(0.0, -0.5, 0.0)));
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(10.0, 0.5, 10.0)
                .friction(config.contact.friction)
                .restitution(config.contact.restitution),
            ground,
            &mut bodies,
        );

        let chassis = bodies.insert(RigidBodyBuilder::dynamic().translation(Vec3::new(
            0.0,
            config.spawn_height_m(),
            0.0,
        )));
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(half_length, half_height, half_width)
                .mass(config.body.mass_kg)
                .friction(config.contact.friction)
                .restitution(config.contact.restitution),
            chassis,
            &mut bodies,
        );

        let mut actuators = Vec::with_capacity(ACTUATOR_COUNT);
        let inset = config.body.hip_inset;
        // Front-right, front-left, rear-right, rear-left.
        let hips = [
            (half_length * inset, half_width * inset),
            (half_length * inset, -half_width * inset),
            (-half_length * inset, half_width * inset),
            (-half_length * inset, -half_width * inset),
        ];

        let hip_stance = config.stance.hip_rad();
        let knee_stance = config.stance.knee_rad();
        let spawn_height = config.spawn_height_m();

        for (hip_x, hip_z) in hips {
            let hip_local = Vec3::new(hip_x, -half_height, hip_z);
            let hip_world = Vec3::new(hip_x, spawn_height - half_height, hip_z);
            let upper_half = config.leg.upper_length_m * 0.5;
            let lower_half = config.leg.lower_length_m * 0.5;

            // Forward kinematics of the crouched stance, in the leg's sagittal
            // plane. A joint angle of zero points the link straight down, so a
            // hip angle rotates the thigh forward and the knee angle folds the
            // shank relative to it.
            let upper_dir = Vec3::new(hip_stance.sin(), -hip_stance.cos(), 0.0);
            let knee_world = hip_world + upper_dir * config.leg.upper_length_m;
            let shank_angle = hip_stance + knee_stance;
            let lower_dir = Vec3::new(shank_angle.sin(), -shank_angle.cos(), 0.0);

            let upper = bodies.insert(
                RigidBodyBuilder::dynamic()
                    .translation(hip_world + upper_dir * upper_half)
                    .rotation(Vec3::Z * hip_stance),
            );
            colliders.insert_with_parent(
                ColliderBuilder::capsule_y(upper_half, radius)
                    .mass(config.leg.upper_mass_kg)
                    .friction(config.contact.friction)
                    .restitution(config.contact.restitution),
                upper,
                &mut bodies,
            );

            let lower = bodies.insert(
                RigidBodyBuilder::dynamic()
                    .translation(knee_world + lower_dir * lower_half)
                    .rotation(Vec3::Z * shank_angle),
            );
            colliders.insert_with_parent(
                ColliderBuilder::capsule_y(lower_half, radius)
                    .mass(config.leg.lower_mass_kg)
                    .friction(config.contact.friction)
                    .restitution(config.contact.restitution),
                lower,
                &mut bodies,
            );

            // Both joints hinge about the lateral axis, so each leg works in
            // its own sagittal plane: hips swing fore and aft, knees fold.
            // ForceBased, not rapier's AccelerationBased default. A real servo
            // is specified by a torque (stall torque, N*m), and only this model
            // interprets stiffness and the force cap in those units. Under the
            // default model `max_torque_nm` is decorative: it can be raised a
            // hundredfold with no effect on the result.
            let hip_joint = RevoluteJointBuilder::new(Vec3::Z)
                .motor_model(MotorModel::ForceBased)
                .local_anchor1(hip_local)
                .local_anchor2(Vec3::new(0.0, upper_half, 0.0))
                .limits([config.servo.min_angle_rad(), config.servo.max_angle_rad()])
                .motor_position(hip_stance, config.servo.stiffness, config.servo.damping)
                .motor_max_force(config.servo.max_torque_nm)
                // Directly jointed links overlap at the hinge by construction.
                // Leaving contacts on makes them shove each other apart.
                .contacts_enabled(false);
            let hip_handle = impulse_joints.insert(chassis, upper, hip_joint, true);

            let knee_joint = RevoluteJointBuilder::new(Vec3::Z)
                .motor_model(MotorModel::ForceBased)
                .local_anchor1(Vec3::new(0.0, -upper_half, 0.0))
                .local_anchor2(Vec3::new(0.0, lower_half, 0.0))
                .limits([config.servo.min_angle_rad(), config.servo.max_angle_rad()])
                .motor_position(knee_stance, config.servo.stiffness, config.servo.damping)
                .motor_max_force(config.servo.max_torque_nm)
                .contacts_enabled(false);
            let knee_handle = impulse_joints.insert(upper, lower, knee_joint, true);

            let leg = actuators.len() / 2;
            actuators.push(Actuator {
                joint: hip_handle,
                parent: chassis,
                child: upper,
                servo: Servo::with_centre(&config.servo, hip_stance),
                calibration_offset_rad: calibration_offsets_rad[leg * 2],
            });
            actuators.push(Actuator {
                joint: knee_handle,
                parent: upper,
                child: lower,
                servo: Servo::with_centre(&config.servo, knee_stance),
                calibration_offset_rad: calibration_offsets_rad[leg * 2 + 1],
            });
        }

        let integration_parameters = IntegrationParameters {
            dt: config.sim.physics_dt(),
            ..IntegrationParameters::default()
        };
        let gravity = Vec3::new(0.0, -config.sim.gravity_m_s2, 0.0);

        Self {
            config,
            bodies,
            colliders,
            impulse_joints,
            multibody_joints: MultibodyJointSet::new(),
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            ccd_solver: CCDSolver::new(),
            integration_parameters,
            gravity,
            chassis,
            actuators,
            tick: 0,
        }
    }

    /// Add the test range's low transverse rails as real collision geometry.
    /// Existing simulation and search callers retain their original flat world.
    pub fn add_test_rails(&mut self) {
        for x in [0.3, 0.55, 0.8] {
            let body = self
                .bodies
                .insert(RigidBodyBuilder::fixed().translation(Vec3::new(x, 0.003, 0.0)));
            self.colliders.insert_with_parent(
                ColliderBuilder::cuboid(0.012, 0.003, 0.3).friction(self.config.contact.friction),
                body,
                &mut self.bodies,
            );
        }
    }

    /// Exact collider geometry and transforms; no inferred gait or cosmetic motion.
    pub fn body_poses(&self) -> Vec<BodyPose> {
        self.colliders
            .iter()
            .filter_map(|(_, collider)| {
                let parent = collider.parent()?;
                let kind = if parent == self.chassis {
                    "chassis"
                } else if self.actuators.iter().any(|a| a.child == parent) {
                    "limb"
                } else if collider
                    .shape()
                    .as_cuboid()
                    .is_some_and(|b| b.half_extents.x > 1.0)
                {
                    "ground"
                } else {
                    "obstacle"
                };
                let (shape, size) = if let Some(b) = collider.shape().as_cuboid() {
                    (
                        "box",
                        [b.half_extents.x, b.half_extents.y, b.half_extents.z],
                    )
                } else if let Some(c) = collider.shape().as_capsule() {
                    ("capsule", [c.radius, c.half_height(), 0.0])
                } else {
                    return None;
                };
                let pose = collider.position();
                let p = pose.translation;
                let q = pose.rotation;
                Some(BodyPose {
                    kind,
                    shape,
                    size,
                    position: [p.x, p.y, p.z],
                    rotation: [q.x, q.y, q.z, q.w],
                })
            })
            .collect()
    }

    pub fn config(&self) -> &RobotConfig {
        &self.config
    }

    pub fn actuator_count(&self) -> usize {
        self.actuators.len()
    }

    /// Runs one control tick: rate-limit every servo command, then integrate
    /// the physics substeps that fall inside that tick.
    ///
    /// Missing or excess actions are treated as "hold centre" rather than an
    /// error, so a controller with the wrong output arity fails visibly in the
    /// trajectory instead of panicking mid-run.
    pub fn control_step(&mut self, actions: &[f32]) -> Sample {
        let control_dt = self.config.sim.control_dt();
        let stiffness = self.config.servo.stiffness;
        let damping = self.config.servo.damping;

        for (index, actuator) in self.actuators.iter_mut().enumerate() {
            let action = actions.get(index).copied().unwrap_or(0.0);
            let commanded = actuator.servo.step(action, control_dt);
            let held = commanded + actuator.calibration_offset_rad;
            if let Some(joint) = self.impulse_joints.get_mut(actuator.joint, true) {
                joint
                    .data
                    .set_motor_position(JointAxis::AngX, held, stiffness, damping);
            }
        }

        for _ in 0..self.config.sim.physics_substeps.max(1) {
            self.pipeline.step(
                self.gravity,
                &self.integration_parameters,
                &mut self.islands,
                &mut self.broad_phase,
                &mut self.narrow_phase,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.ccd_solver,
                &(),
                &(),
            );
        }

        self.tick += 1;
        self.sample()
    }

    /// The angles the servos currently believe they hold, in actuator order.
    ///
    /// Accumulating the change in these across a run gives total commanded
    /// joint travel, which is the effort proxy the search bins on.
    pub fn commanded_angles(&self) -> Vec<f32> {
        self.actuators
            .iter()
            .map(|actuator| actuator.servo.commanded_rad())
            .collect()
    }

    pub fn sample(&self) -> Sample {
        let chassis = &self.bodies[self.chassis];
        let translation = chassis.translation();
        let rotation = chassis.rotation();

        let up = rotation * Vec3::Y;
        let forward = rotation * Vec3::X;

        Sample {
            tick: self.tick,
            time_s: self.tick as f32 * self.config.sim.control_dt(),
            position_m: [translation.x, translation.y, translation.z],
            tilt_rad: up.y.clamp(-1.0, 1.0).acos(),
            pitch_rad: forward.y.clamp(-1.0, 1.0).asin(),
            heading_rad: forward.z.atan2(forward.x),
            joint_rad: self
                .actuators
                .iter()
                .map(|actuator| self.joint_angle(actuator))
                .collect(),
        }
    }

    /// The measured hinge angle, read back from body orientations rather than
    /// from the command. A servo that stalls against the ground reports the
    /// angle it actually reached, which is the whole point of measuring.
    fn joint_angle(&self, actuator: &Actuator) -> f32 {
        let parent = self.bodies[actuator.parent].rotation();
        let child = self.bodies[actuator.child].rotation();
        twist_about_z(parent.inverse() * child)
    }

    /// Steps with no actuation until the robot stops moving, or the budget
    /// runs out. Used to settle a drop before a gait starts.
    pub fn settle(&mut self, max_ticks: usize) -> Sample {
        let hold = vec![0.0; self.actuator_count()];
        let mut previous = self.sample();
        for _ in 0..max_ticks {
            let current = self.control_step(&hold);
            let moved = (current.position_m[0] - previous.position_m[0]).abs()
                + (current.position_m[1] - previous.position_m[1]).abs()
                + (current.position_m[2] - previous.position_m[2]).abs();
            previous = current;
            if moved < 1e-5 {
                break;
            }
        }
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTUATOR_COUNT, FieldSim, twist_about_z};
    use crate::config::RobotConfig;
    use rapier3d::prelude::*;

    #[test]
    fn twist_reads_back_a_rotation_about_the_hinge_axis() {
        let angle = 0.7_f32;
        let rotation = Rotation::from_axis_angle(Vec3::Z, angle);

        assert!((twist_about_z(rotation) - angle).abs() < 1e-5);
    }

    #[test]
    fn the_robot_has_one_servo_per_joint() {
        let sim = FieldSim::new(RobotConfig::nominal());
        assert_eq!(sim.actuator_count(), ACTUATOR_COUNT);
    }

    #[test]
    fn a_dropped_robot_settles_to_a_physically_sane_pose() {
        let config = RobotConfig::nominal();
        let mut sim = FieldSim::new(config.clone());

        let settled = sim.settle(400);

        // It must come to rest above the floor, not through it.
        assert!(
            settled.height_m() > 0.0,
            "chassis fell through the floor: {settled:?}"
        );
        // And no higher than it started, since nothing pushes it up.
        assert!(
            settled.height_m() <= config.spawn_height_m() + 1e-3,
            "chassis gained height while unactuated: {settled:?}"
        );
        // A passive drop must not wander off sideways or forwards.
        assert!(settled.forward_m().abs() < 0.05, "{settled:?}");
        assert!(settled.lateral_m().abs() < 0.05, "{settled:?}");
        // And it must not end up on its back.
        assert!(
            settled.tilt_rad < 1.0,
            "chassis tumbled: tilt {} rad",
            settled.tilt_rad
        );
    }

    #[test]
    fn an_unactuated_robot_does_not_drive_itself_forward() {
        // The failure mode this whole crate exists to avoid: motion that comes
        // from a term in an equation rather than from contact.
        let mut sim = FieldSim::new(RobotConfig::nominal());
        let hold = vec![0.0; ACTUATOR_COUNT];

        for _ in 0..200 {
            sim.control_step(&hold);
        }

        let sample = sim.sample();
        assert!(
            sample.forward_m().abs() < 0.05,
            "unactuated robot travelled {} m",
            sample.forward_m()
        );
    }

    #[test]
    fn identical_inputs_produce_identical_trajectories() {
        let run = || {
            let mut sim = FieldSim::new(RobotConfig::nominal());
            let mut samples = Vec::new();
            for tick in 0..60 {
                let phase = tick as f32 * 0.3;
                let actions: Vec<f32> = (0..ACTUATOR_COUNT)
                    .map(|index| (phase + index as f32).sin() * 0.6)
                    .collect();
                samples.push(sim.control_step(&actions));
            }
            samples
        };

        assert_eq!(run(), run());
    }

    #[test]
    fn actions_outside_the_actuator_count_are_ignored_rather_than_fatal() {
        let mut sim = FieldSim::new(RobotConfig::nominal());

        let short = sim.control_step(&[0.5]);
        let long = sim.control_step(&[0.5; ACTUATOR_COUNT + 4]);

        assert_eq!(short.joint_rad.len(), ACTUATOR_COUNT);
        assert_eq!(long.joint_rad.len(), ACTUATOR_COUNT);
    }
}
