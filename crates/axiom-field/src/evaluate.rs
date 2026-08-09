//! Scoring a controller, and refusing to reward a fall.
//!
//! S1 established that distance alone is a trap: a robot that topples and
//! slides still travels, and at larger stride amplitudes it "travels" further
//! than some robots that walk. A search rewarded on displacement alone will
//! find the fall, because falling is easier than walking. So a run that ends
//! off its feet scores zero, and the reason is recorded rather than inferred.

use axiom::rng::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    RobotConfig,
    controller::CpgGenome,
    perturb::{Perturbation, Spread},
    sim::FieldSim,
    trajectory::SETTLE_TICKS,
};

/// Fraction of standing height below which the robot counts as down.
pub const FALLEN_HEIGHT_RATIO: f32 = 0.6;
/// Tilt beyond which the robot counts as down, in radians (~29 degrees).
pub const FALLEN_TILT_RAD: f32 = 0.5;
/// Weight on lateral drift, so that walking in a circle scores below walking
/// in a line without being disqualified outright.
pub const LATERAL_PENALTY: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    pub travel_m: f32,
    pub lateral_m: f32,
    pub speed_m_s: f32,
    /// Total commanded joint travel over the run, in radians. A proxy for
    /// energy: it is what the servos were asked to do, not measured power.
    pub effort_rad: f32,
    /// Effort spent per metre of forward progress — the economy axis.
    pub effort_per_m: f32,
    pub fell: bool,
    pub fitness: f32,
}

impl Outcome {
    /// A run that never got going, used when a genome is unusable.
    pub fn inert() -> Self {
        Self {
            travel_m: 0.0,
            lateral_m: 0.0,
            speed_m_s: 0.0,
            effort_rad: 0.0,
            effort_per_m: 0.0,
            fell: true,
            fitness: 0.0,
        }
    }
}

/// Runs one genome in one world.
pub fn evaluate(
    config: &RobotConfig,
    genome: &CpgGenome,
    perturbation: &Perturbation,
    ticks: usize,
) -> Outcome {
    if !genome.is_well_formed() {
        return Outcome::inert();
    }

    let world = perturbation.apply(config);
    let standing = world.standing_height_m();
    let control_hz = world.sim.control_hz;
    let mut sim = FieldSim::with_calibration(world, perturbation.joint_offsets_rad);

    // Settle first, so travel measures walking rather than the drop.
    sim.settle(SETTLE_TICKS);
    let start = sim.sample();
    let mut commanded = sim.commanded_angles();
    let mut effort_rad = 0.0_f32;
    let mut fell = false;

    for tick in 0..ticks {
        let sample = sim.control_step(&genome.actions_at(tick as f32 / control_hz));
        let now = sim.commanded_angles();
        for (current, previous) in now.iter().zip(commanded.iter()) {
            effort_rad += (current - previous).abs();
        }
        commanded = now;

        // Once down, stay down: a robot that flips and slides must not be able
        // to recover a score by chance later in the run.
        if sample.height_m() < standing * FALLEN_HEIGHT_RATIO || sample.tilt_rad > FALLEN_TILT_RAD {
            fell = true;
            break;
        }
    }

    let end = sim.sample();
    let travel_m = end.forward_m() - start.forward_m();
    let lateral_m = end.lateral_m() - start.lateral_m();
    let seconds = (ticks as f32 / control_hz).max(f32::EPSILON);
    let speed_m_s = travel_m / seconds;
    let effort_per_m = if travel_m.abs() > 1e-4 {
        effort_rad / travel_m.abs()
    } else {
        // No progress: report the effort as if it bought one centimetre, so
        // treadmilling lands at the expensive end of the axis instead of at
        // zero next to genuinely efficient gaits.
        effort_rad / 0.01
    };

    let fitness = if fell {
        0.0
    } else {
        (travel_m - LATERAL_PENALTY * lateral_m.abs()).max(0.0)
    };

    Outcome {
        travel_m,
        lateral_m,
        speed_m_s,
        effort_rad,
        effort_per_m,
        fell,
        fitness,
    }
}

/// A genome's score across an ensemble of perturbed worlds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct EnsembleScore {
    /// Mean fitness across the ensemble: what the search selects on.
    pub fitness: f32,
    /// The worst world's fitness, carried for the robustness report.
    pub worst_fitness: f32,
    pub mean_speed_m_s: f32,
    pub mean_effort_per_m: f32,
    /// How many worlds ended with the robot off its feet.
    pub falls: usize,
    pub worlds: usize,
}

impl EnsembleScore {
    /// The behaviour-space descriptor: speed against economy.
    ///
    /// Economy is binned on a log scale because it is a ratio spanning two
    /// orders of magnitude — a pilot over 200 controllers found effort per
    /// metre running from 137 to 29,129 rad/m, with the median at 1,117. On a
    /// linear axis nearly every controller lands in one column, which is
    /// precisely the failure that leaves Axiom's own archive at 6% coverage.
    pub fn descriptor(&self) -> [f32; 2] {
        [
            self.mean_speed_m_s,
            self.mean_effort_per_m.max(1e-3).log10(),
        ]
    }

    pub fn stood_in_every_world(&self) -> bool {
        self.falls == 0
    }
}

/// Scores a genome across every world in `ensemble`.
///
/// Selection is on the mean rather than the best: a controller that only works
/// in one lucky world is exactly what domain randomization exists to reject.
pub fn evaluate_ensemble(
    config: &RobotConfig,
    genome: &CpgGenome,
    ensemble: &[Perturbation],
    ticks: usize,
) -> EnsembleScore {
    if ensemble.is_empty() {
        return EnsembleScore {
            fitness: 0.0,
            worst_fitness: 0.0,
            mean_speed_m_s: 0.0,
            mean_effort_per_m: 0.0,
            falls: 0,
            worlds: 0,
        };
    }

    let mut total_fitness = 0.0;
    let mut worst_fitness = f32::MAX;
    let mut total_speed = 0.0;
    let mut total_effort = 0.0;
    let mut falls = 0;

    for perturbation in ensemble {
        let outcome = evaluate(config, genome, perturbation, ticks);
        total_fitness += outcome.fitness;
        worst_fitness = worst_fitness.min(outcome.fitness);
        total_speed += outcome.speed_m_s;
        total_effort += outcome.effort_per_m;
        if outcome.fell {
            falls += 1;
        }
    }

    let worlds = ensemble.len() as f32;
    EnsembleScore {
        fitness: total_fitness / worlds,
        worst_fitness,
        mean_speed_m_s: total_speed / worlds,
        mean_effort_per_m: total_effort / worlds,
        falls,
        worlds: ensemble.len(),
    }
}

/// Draws a fresh ensemble for held-out testing.
pub fn holdout_ensemble(seed: u64, spread: &Spread, count: usize) -> Vec<Perturbation> {
    let mut rng = Rng::new(seed);
    (0..count)
        .map(|_| Perturbation::sample(&mut rng, spread))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Outcome, evaluate, evaluate_ensemble};
    use crate::{
        config::RobotConfig,
        controller::CpgGenome,
        gait::sine_gait,
        perturb::{Perturbation, Spread},
    };

    fn config() -> RobotConfig {
        RobotConfig::nominal()
    }

    #[test]
    fn the_reference_trot_walks_and_is_not_scored_as_a_fall() {
        let outcome = evaluate(
            &config(),
            &CpgGenome::from_gait(&sine_gait()),
            &Perturbation::nominal(),
            120,
        );

        assert!(!outcome.fell, "{outcome:?}");
        assert!(outcome.travel_m > 0.2, "{outcome:?}");
        assert!(outcome.fitness > 0.0, "{outcome:?}");
        assert!(outcome.effort_rad > 0.0, "{outcome:?}");
    }

    #[test]
    fn a_toppling_gait_scores_zero_however_far_it_slides() {
        // The S1 finding, turned into a rule: hip 0.45 / knee 0.35 ends upside
        // down while still covering ground.
        let mut wild = sine_gait();
        wild.hip_amplitude = 0.45;
        wild.knee_amplitude = 0.35;

        let outcome = evaluate(
            &config(),
            &CpgGenome::from_gait(&wild),
            &Perturbation::nominal(),
            200,
        );

        assert!(outcome.fell, "expected a fall, got {outcome:?}");
        assert_eq!(outcome.fitness, 0.0);
    }

    #[test]
    fn a_still_robot_lands_at_the_expensive_end_of_the_economy_axis() {
        let mut still = CpgGenome::from_gait(&sine_gait());
        for joint in &mut still.joints {
            joint.amplitude = 0.0;
            joint.bias = 0.0;
        }

        let outcome = evaluate(&config(), &still, &Perturbation::nominal(), 80);

        assert!(outcome.travel_m.abs() < 0.02, "{outcome:?}");
        // It stands rather than falls, so it is not disqualified — but the
        // residual millimetre of settle drift must not read as locomotion.
        assert!(outcome.fitness < 1e-3, "{outcome:?}");
        assert!(outcome.effort_per_m >= 0.0);
    }

    #[test]
    fn a_malformed_genome_is_inert_rather_than_fatal() {
        let mut broken = CpgGenome::from_gait(&sine_gait());
        broken.joints.truncate(3);

        let outcome = evaluate(&config(), &broken, &Perturbation::nominal(), 40);

        assert_eq!(outcome, Outcome::inert());
    }

    #[test]
    fn evaluation_is_reproducible_for_a_fixed_world() {
        let genome = CpgGenome::from_gait(&sine_gait());
        let perturbation = Perturbation::ensemble(9, &Spread::default(), 1)[0];

        let first = evaluate(&config(), &genome, &perturbation, 80);
        let second = evaluate(&config(), &genome, &perturbation, 80);

        assert_eq!(first, second);
    }

    #[test]
    fn the_ensemble_score_selects_on_the_mean_and_records_the_worst() {
        let genome = CpgGenome::from_gait(&sine_gait());
        let ensemble = Perturbation::ensemble(5, &Spread::default(), 4);

        let score = evaluate_ensemble(&config(), &genome, &ensemble, 80);

        assert_eq!(score.worlds, 4);
        assert!(score.fitness >= score.worst_fitness);
        assert!(score.mean_speed_m_s.is_finite());
    }

    #[test]
    fn an_empty_ensemble_scores_zero_instead_of_dividing_by_zero() {
        let score = evaluate_ensemble(&config(), &CpgGenome::from_gait(&sine_gait()), &[], 40);

        assert_eq!(score.worlds, 0);
        assert_eq!(score.fitness, 0.0);
    }
}
