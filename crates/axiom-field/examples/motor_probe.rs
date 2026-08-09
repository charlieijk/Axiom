//! Isolation of a finding that shapes the whole model.
//!
//! rapier's default `MotorModel::AccelerationBased` interprets motor stiffness
//! and the force cap in units that are not N*m, and with light links it barely
//! moves a loaded joint: a servo commanded to 0.8 rad reaches 0.02. Under that
//! model `max_torque_nm` in robot.toml is decorative — raising it a
//! hundredfold changes nothing. `ForceBased` reaches 0.79 and is the model a
//! servo datasheet is actually written for.
//!
//! Run with: cargo run -p axiom-field --example motor_probe
use rapier3d::prelude::*;

fn run(with_limits: bool, model: MotorModel, label: &str) {
    let mut bodies = RigidBodySet::new();
    let mut colliders = ColliderSet::new();
    let mut joints = ImpulseJointSet::new();

    let anchor = bodies.insert(RigidBodyBuilder::fixed().translation(Vec3::new(0.0, 1.0, 0.0)));
    let arm = bodies.insert(RigidBodyBuilder::dynamic().translation(Vec3::new(0.0, 0.9, 0.0)));
    colliders.insert_with_parent(
        ColliderBuilder::capsule_y(0.1, 0.01).mass(0.02),
        arm,
        &mut bodies,
    );

    let mut builder = RevoluteJointBuilder::new(Vec3::Z)
        .local_anchor1(Vec3::ZERO)
        .local_anchor2(Vec3::new(0.0, 0.1, 0.0))
        .motor_model(model)
        .motor_position(0.8, 6.0, 0.35)
        .motor_max_force(0.15);
    if with_limits {
        builder = builder.limits([-1.309, 1.309]);
    }
    joints.insert(anchor, arm, builder, true);

    let mut pipeline = PhysicsPipeline::new();
    let mut islands = IslandManager::new();
    let mut bp = DefaultBroadPhase::new();
    let mut np = NarrowPhase::new();
    let mut ccd = CCDSolver::new();
    let mut mj = MultibodyJointSet::new();
    let params = IntegrationParameters {
        dt: 0.005,
        ..Default::default()
    };

    for _ in 0..400 {
        pipeline.step(
            Vec3::new(0.0, -9.81, 0.0),
            &params,
            &mut islands,
            &mut bp,
            &mut np,
            &mut bodies,
            &mut colliders,
            &mut joints,
            &mut mj,
            &mut ccd,
            &(),
            &(),
        );
    }
    let r = bodies[arm].rotation();
    let angle = 2.0 * r.z.atan2(r.w);
    println!("{label:28} target=0.800 reached={angle:+.4}");
}

fn main() {
    run(
        false,
        MotorModel::AccelerationBased,
        "no limits, accel-based",
    );
    run(
        true,
        MotorModel::AccelerationBased,
        "with limits, accel-based",
    );
    run(false, MotorModel::ForceBased, "no limits, force-based");
    run(true, MotorModel::ForceBased, "with limits, force-based");
}
