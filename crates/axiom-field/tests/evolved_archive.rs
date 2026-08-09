//! The committed archive from a full search.
//!
//! A complete run is 576 genomes across 4 worlds — about a hundred seconds in
//! release and far too slow to re-run in CI, so the result is committed as an
//! artifact and its properties are gated here. Reproducing it is a separate
//! guarantee, covered by the seeded-determinism tests in `search.rs`.
//!
//! Regenerate with:
//!   cargo run --release -p axiom-field -- evolve --seed 1 --generations 24 \
//!     --batch 24 --out crates/axiom-field/tests/golden/archive-seed1.json

use axiom_field::{RobotConfig, SearchReport};

const ARCHIVE: &str = include_str!("golden/archive-seed1.json");

fn archive() -> SearchReport {
    SearchReport::from_json(ARCHIVE).expect("the committed archive must parse")
}

#[test]
fn the_archive_covers_a_real_share_of_the_behaviour_space() {
    // Coverage is the headline quality-diversity number, and it is the one
    // most easily faked by choosing axis ranges nothing can reach. These
    // ranges came from `axiom-field pilot`; Axiom's own guessed ranges leave
    // its archive at 6%.
    let report = archive();

    assert!(
        report.coverage() > 0.20,
        "coverage {:.1}% is below the 20% gate",
        report.coverage() * 100.0
    );
    assert!(report.archive.occupied_count() > 20);
}

#[test]
fn every_archived_controller_is_a_gait_and_not_a_fall() {
    // The S1 lesson, enforced on the artifact: a controller that goes down in
    // every world is not a behaviour worth keeping, however far it slid.
    let report = archive();

    for elite in report.archive.occupants() {
        assert!(
            elite.score.falls < elite.score.worlds,
            "cell {:?} fell in all {} worlds",
            elite.cell,
            elite.score.worlds
        );
    }
}

#[test]
fn the_best_controller_stands_in_every_training_world() {
    let report = archive();
    let best = report.best().expect("a populated archive has a best");

    assert_eq!(
        best.score.falls, 0,
        "the best controller fell in {} of {} worlds",
        best.score.falls, best.score.worlds
    );
    assert!(best.score.mean_speed_m_s > 0.09, "{:?}", best.score);
    // Selection is on the mean, so the worst world is the honest floor.
    assert!(best.score.worst_fitness > 0.0, "{:?}", best.score);
}

#[test]
fn coverage_grew_monotonically_through_the_run() {
    let report = archive();

    for pair in report.history.windows(2) {
        assert!(
            pair[1].occupied >= pair[0].occupied,
            "generation {} lost cells: {} -> {}",
            pair[1].generation,
            pair[0].occupied,
            pair[1].occupied
        );
    }
    let first = report.history.first().expect("history is recorded");
    let last = report.history.last().expect("history is recorded");
    assert!(
        last.occupied > first.occupied,
        "the search added no cells after its first generation"
    );
}

#[test]
fn the_archive_records_which_robot_it_was_searched_against() {
    let report = archive();
    let config = RobotConfig::nominal();

    assert_eq!(report.robot, config.meta.name);
    assert_eq!(report.seed, 1);
    // These controllers were evolved against parameters nobody has measured.
    // If calibration ever lands, this archive describes a different robot.
    assert_eq!(
        report.calibrated, config.meta.calibrated,
        "calibration state changed; re-run the search"
    );
    assert!(!report.calibrated);
}
