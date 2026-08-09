//! The committed reference run.
//!
//! This is the regression anchor for the whole model: a known open-loop input
//! producing a known trajectory. Any change to the geometry, the servo model,
//! the contact parameters, or the solver configuration moves these numbers,
//! and this test is what forces that change to be deliberate.
//!
//! Re-record with:
//!   cargo run -p axiom-field -- record \
//!     --out crates/axiom-field/tests/golden/trot-nominal.json --ticks 200 --stride 10

use axiom_field::{RobotConfig, Trajectory, gait::sine_gait};

const GOLDEN: &str = include_str!("golden/trot-nominal.json");

fn golden() -> Trajectory {
    Trajectory::from_json(GOLDEN).expect("the committed golden must parse")
}

#[test]
fn the_reference_trot_replays_within_tolerance() {
    let golden = golden();
    let replay = Trajectory::record(
        RobotConfig::nominal(),
        sine_gait(),
        golden.ticks,
        golden.stride,
    );

    if let Err(divergence) = replay.compare(&golden) {
        panic!("the model no longer reproduces the committed reference run: {divergence}");
    }
}

#[test]
fn the_golden_was_recorded_against_the_bundled_parameters() {
    let golden = golden();
    let config = RobotConfig::nominal();

    assert_eq!(golden.robot, config.meta.name);
    assert_eq!(golden.control_hz, config.sim.control_hz);
    // If the parameters ever become calibrated, this recording describes the
    // old uncalibrated robot and must be re-recorded rather than trusted.
    assert_eq!(
        golden.calibrated, config.meta.calibrated,
        "calibration state changed; re-record the golden trajectory"
    );
}

#[test]
fn the_reference_run_is_a_walk_and_not_a_fall() {
    // A trajectory that travels because the robot toppled and slid would still
    // replay deterministically, so determinism alone is not enough: the
    // reference run has to be locomotion for the comparison to mean anything.
    let golden = golden();
    let config = RobotConfig::nominal();
    let standing = config.standing_height_m();

    let first = golden.samples.first().expect("golden has samples");
    let last = golden.samples.last().expect("golden has samples");

    assert!(
        last.height_m() > standing * 0.9,
        "the robot ended at {:.4} m against a standing height of {standing:.4} m",
        last.height_m()
    );
    assert!(
        last.tilt_rad < 0.2,
        "the robot ended tilted {:.3} rad",
        last.tilt_rad
    );
    assert!(
        golden.forward_travel_m() > 0.5,
        "the reference run only travelled {:.4} m",
        golden.forward_travel_m()
    );
    // Straight-ish: lateral drift well under forward travel.
    let lateral = (last.lateral_m() - first.lateral_m()).abs();
    assert!(
        lateral < golden.forward_travel_m() * 0.25,
        "lateral drift {lateral:.4} m is large next to {:.4} m of travel",
        golden.forward_travel_m()
    );
}

#[test]
fn a_changed_model_is_caught_by_the_golden() {
    // Guards the guard: if the tolerance were loose enough to pass anything,
    // this crate's central regression test would be worthless. Two percent of
    // chassis mass is well below any plausible measurement error, and it must
    // still be visible.
    let golden = golden();
    let mut altered = RobotConfig::nominal();
    altered.body.mass_kg *= 1.02;

    let replay = Trajectory::record(altered, sine_gait(), golden.ticks, golden.stride);

    replay
        .compare(&golden)
        .expect_err("a 2% chassis mass change must break the golden comparison");
}

#[test]
fn the_reference_gait_never_saturates_the_servo_slew_rate() {
    // Worth knowing before any hardware comparison: at the committed
    // amplitudes the gait never asks a servo to move faster than roughly half
    // its rated speed, so the slew limit is not what shapes this trajectory.
    // Halving the rated speed changes the result not at all. A gait that did
    // saturate would diverge sharply on hardware, whose loaded speed is well
    // below its datasheet figure.
    let config = RobotConfig::nominal();
    let gait = sine_gait();
    let dt = config.sim.control_dt();
    let half_span = 0.5 * (config.servo.max_angle_rad() - config.servo.min_angle_rad());

    let mut peak_rad_per_s = 0.0_f32;
    let mut previous = gait.actions_at(0.0);
    for tick in 1..200 {
        let current = gait.actions_at(tick as f32 * dt);
        for (now, before) in current.iter().zip(previous.iter()) {
            peak_rad_per_s = peak_rad_per_s.max((now - before).abs() * half_span / dt);
        }
        previous = current;
    }

    let rated = config.servo.max_rate_rad_per_s();
    assert!(
        peak_rad_per_s < rated,
        "gait demands {peak_rad_per_s:.2} rad/s against a rated {rated:.2} rad/s"
    );
    assert!(
        peak_rad_per_s > rated * 0.25,
        "gait demands only {peak_rad_per_s:.2} rad/s of {rated:.2}; the headroom claim is stale"
    );
}
