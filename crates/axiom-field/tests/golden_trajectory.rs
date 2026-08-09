//! The committed reference run, and what it can honestly prove.
//!
//! Legged contact is close to chaotic. The same commit built on x86_64 and on
//! aarch64 diverges by about 10 mm within two seconds of walking, and further
//! after that, so a committed file cannot anchor pointwise position across
//! machines. It anchors *behaviour*: the robot walks, forward, upright, about
//! this far.
//!
//! Sharp detection of small parameter changes needs two runs from one binary,
//! where the comparison is exact. Both kinds of test live here so the
//! difference is visible rather than assumed.
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
fn the_reference_trot_still_walks_like_the_committed_run() {
    let golden = golden();
    let expected = golden.summary();
    let replay = Trajectory::record(
        RobotConfig::nominal(),
        sine_gait(),
        golden.ticks,
        golden.stride,
    )
    .summary();

    // Bands sized for cross-architecture solver noise, not for precision.
    // A 2% chassis-mass change moves travel by ~3.8%, so this will not catch
    // one; that is the job of the same-binary test below.
    let travel_band = expected.forward_travel_m.abs() * 0.25;
    assert!(
        (replay.forward_travel_m - expected.forward_travel_m).abs() < travel_band,
        "travel {:.4} m against a committed {:.4} m",
        replay.forward_travel_m,
        expected.forward_travel_m
    );
    assert!(
        (replay.final_height_m - expected.final_height_m).abs() < 0.02,
        "final height {:.4} m against a committed {:.4} m",
        replay.final_height_m,
        expected.final_height_m
    );
    assert!(
        replay.max_tilt_rad < expected.max_tilt_rad.max(0.05) * 3.0,
        "peak tilt {:.4} rad against a committed {:.4} rad",
        replay.max_tilt_rad,
        expected.max_tilt_rad
    );
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
    // replay deterministically, so determinism alone proves nothing: the
    // reference run has to be locomotion for any of this to mean anything.
    let golden = golden();
    let standing = RobotConfig::nominal().standing_height_m();
    let summary = golden.summary();

    assert!(
        summary.final_height_m > standing * 0.9,
        "the robot ended at {:.4} m against a standing height of {standing:.4} m",
        summary.final_height_m
    );
    assert!(
        summary.max_tilt_rad < 0.2,
        "the robot peaked at {:.3} rad of tilt",
        summary.max_tilt_rad
    );
    assert!(
        summary.forward_travel_m > 0.5,
        "the reference run only travelled {:.4} m",
        summary.forward_travel_m
    );
    assert!(
        summary.lateral_drift_m.abs() < summary.forward_travel_m * 0.25,
        "lateral drift {:.4} m is large next to {:.4} m of travel",
        summary.lateral_drift_m,
        summary.forward_travel_m
    );
}

#[test]
fn a_small_parameter_change_is_detected_within_one_binary() {
    // The sharp guard. Both runs come from this binary, so the comparison is
    // exact and architecture-independent — which is exactly why it can afford
    // a perturbation far below any plausible measurement error.
    let baseline = Trajectory::record(RobotConfig::nominal(), sine_gait(), 200, 10);

    let mut altered = RobotConfig::nominal();
    altered.body.mass_kg *= 1.02;
    let changed = Trajectory::record(altered, sine_gait(), 200, 10);

    changed
        .compare(&baseline)
        .expect_err("a 2% chassis mass change must be detectable");
}

#[test]
fn the_reference_gait_exercises_geometry_but_not_the_servo_limits() {
    // A load-bearing caveat for any future hardware comparison. At the
    // committed amplitudes, halving the rated slew rate or the stall torque
    // changes the trajectory not at all — the gait never demands either. The
    // servo parameters most likely to be wrong on real hardware are therefore
    // the ones this reference run cannot discriminate, and a transfer study
    // needs a faster or heavier-loaded gait to probe them.
    let baseline = Trajectory::record(RobotConfig::nominal(), sine_gait(), 120, 10);

    for perturb in [
        |config: &mut RobotConfig| config.servo.max_rate_deg_per_s *= 0.5,
        |config: &mut RobotConfig| config.servo.max_torque_nm *= 0.5,
    ] {
        let mut altered = RobotConfig::nominal();
        perturb(&mut altered);
        let run = Trajectory::record(altered, sine_gait(), 120, 10);
        assert_eq!(
            run.samples, baseline.samples,
            "a servo limit that the gait never reaches changed the result; \
             the headroom note in gait.rs is stale"
        );
    }
}
