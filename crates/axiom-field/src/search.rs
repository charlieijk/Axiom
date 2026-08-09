//! MAP-Elites over the field simulator.
//!
//! The archive keeps the best controller per behaviour cell rather than one
//! winner, so the output is a repertoire of gaits along a speed-against-economy
//! trade-off. That is the point of quality-diversity on hardware: when a real
//! robot turns out to differ from its model, having twelve viable gaits to try
//! is worth more than having one optimum tuned to a fiction.
//!
//! The grid mechanics come from `axiom::qd::Grid`, which carries no Axiom
//! domain types — this crate supplies its own payload and its own axes.

use axiom::{
    qd::{AxisSpec, Grid},
    rng::Rng,
};
use serde::{Deserialize, Serialize};

use crate::{
    config::RobotConfig,
    controller::CpgGenome,
    evaluate::{EnsembleScore, evaluate_ensemble},
    gait::sine_gait,
    perturb::{Perturbation, Spread},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FieldElite {
    pub genome: CpgGenome,
    pub score: EnsembleScore,
    pub cell: (usize, usize),
    pub generation: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GenerationRecord {
    pub generation: usize,
    pub best_fitness: f32,
    pub coverage: f32,
    pub occupied: usize,
    /// Genomes in this generation that fell in at least one world.
    pub fell_somewhere: usize,
}

#[derive(Clone, Debug)]
pub struct SearchConfig {
    pub seed: u64,
    pub generations: usize,
    /// Genomes evaluated per generation.
    pub batch: usize,
    pub ticks: usize,
    /// Worlds in the training ensemble.
    pub worlds: usize,
    pub spread: Spread,
    pub archive_width: usize,
    pub archive_height: usize,
    pub speed_axis: AxisSpec,
    pub effort_axis: AxisSpec,
    pub mutation_scale: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            generations: 24,
            batch: 24,
            ticks: 160,
            worlds: 4,
            spread: Spread::default(),
            archive_width: 12,
            archive_height: 8,
            // Ranges measured with `axiom-field pilot` over 200 controllers,
            // not guessed. Axiom's own archive shows what guessing costs: a
            // hardcoded 0..12 m range over real distances of ~2 m leaves ten
            // of twelve columns unreachable and coverage stuck at 6%. The
            // first guesses here were worse — a 0..0.30 speed range against a
            // measured p95 of 0.074, and a linear 0..400 effort range against
            // a measured spread of 137..29,129.
            //
            // Speed starts at zero: a controller that walks backwards earns no
            // fitness, so it lands in the first column and is outcompeted
            // there rather than being given cells of its own.
            speed_axis: AxisSpec::new("forward speed (m/s)", 0.0, 0.15),
            effort_axis: AxisSpec::new("log10 effort per metre (rad/m)", 2.0, 4.5),
            mutation_scale: 0.15,
        }
    }
}

impl SearchConfig {
    /// The training ensemble, fixed for the whole run.
    ///
    /// Fixed rather than resampled per evaluation so that two genomes are
    /// always compared on the same worlds; resampling would make the archive a
    /// record of which genomes drew easy worlds. The price is that the search
    /// can overfit to these particular worlds, which is exactly what the
    /// held-out ensemble later measures.
    pub fn training_ensemble(&self) -> Vec<Perturbation> {
        Perturbation::ensemble(self.seed ^ 0x5eed, &self.spread, self.worlds)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchReport {
    pub robot: String,
    pub calibrated: bool,
    pub seed: u64,
    pub generations: usize,
    pub evaluated: usize,
    pub ticks: usize,
    pub worlds: usize,
    pub archive: Grid<FieldElite>,
    pub history: Vec<GenerationRecord>,
}

impl SearchReport {
    pub fn coverage(&self) -> f32 {
        self.archive.coverage()
    }

    pub fn best(&self) -> Option<&FieldElite> {
        self.archive.occupants().max_by(|a, b| {
            a.score
                .fitness
                .partial_cmp(&b.score.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("search report serialization cannot fail")
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Runs MAP-Elites and returns the archive plus its history.
pub fn run_search(config: &SearchConfig, robot: &RobotConfig) -> SearchReport {
    let mut rng = Rng::new(config.seed);
    let ensemble = config.training_ensemble();
    let mut archive: Grid<FieldElite> = Grid::new(config.archive_width, config.archive_height);
    let mut history = Vec::with_capacity(config.generations);
    let mut evaluated = 0;

    let seed_genome = CpgGenome::from_gait(&sine_gait());

    for generation in 0..config.generations {
        let mut fell_somewhere = 0;

        for index in 0..config.batch {
            let candidate = match archive.sample(&mut rng) {
                // Once the archive has occupants, vary them.
                Some(elite) => elite.genome.mutate(&mut rng, config.mutation_scale),
                // The first generation is half the known-good trot under
                // increasing mutation and half random, so the search neither
                // starts blind nor only ever circles the seed.
                None => {
                    if index % 2 == 0 {
                        seed_genome.mutate(&mut rng, config.mutation_scale * (1.0 + index as f32))
                    } else {
                        CpgGenome::random(&mut rng)
                    }
                }
            };

            let score = evaluate_ensemble(robot, &candidate, &ensemble, config.ticks);
            evaluated += 1;
            if score.falls > 0 {
                fell_somewhere += 1;
            }

            // A controller that falls in every world has no behaviour worth
            // archiving; letting it occupy a cell would crowd out a gait.
            if score.falls == score.worlds {
                continue;
            }

            let cell = archive.cell_for(
                (&config.speed_axis, &config.effort_axis),
                score.descriptor(),
            );
            archive.insert_better(
                cell,
                FieldElite {
                    genome: candidate,
                    score,
                    cell,
                    generation,
                },
                |elite| elite.score.fitness,
            );
        }

        let best_fitness = archive
            .occupants()
            .map(|elite| elite.score.fitness)
            .fold(0.0_f32, f32::max);
        history.push(GenerationRecord {
            generation,
            best_fitness,
            coverage: archive.coverage(),
            occupied: archive.occupied_count(),
            fell_somewhere,
        });
    }

    SearchReport {
        robot: robot.meta.name.clone(),
        calibrated: robot.meta.calibrated,
        seed: config.seed,
        generations: config.generations,
        evaluated,
        ticks: config.ticks,
        worlds: config.worlds,
        archive,
        history,
    }
}

/// How well the archive holds up on worlds it never trained on.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HoldoutReport {
    pub tested: usize,
    /// Elites that still stood in every held-out world.
    pub robust: usize,
    /// Mean ratio of held-out fitness to training fitness, over elites whose
    /// training fitness was non-trivial.
    pub retained_fraction: f32,
    pub per_cell: Vec<HoldoutCell>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HoldoutCell {
    pub cell: (usize, usize),
    pub training_fitness: f32,
    pub holdout_fitness: f32,
    pub holdout_falls: usize,
}

impl HoldoutReport {
    pub fn robust_fraction(&self) -> f32 {
        if self.tested == 0 {
            0.0
        } else {
            self.robust as f32 / self.tested as f32
        }
    }
}

/// Re-scores every elite against worlds drawn from a seed the search never saw.
///
/// This is the software-only stand-in for a transfer test. An elite whose
/// speed collapses here has overfitted the particular worlds it trained on,
/// which is the failure mode that ruins transfer to hardware.
pub fn holdout(
    report: &SearchReport,
    robot: &RobotConfig,
    spread: &Spread,
    seed: u64,
    worlds: usize,
) -> HoldoutReport {
    let ensemble = Perturbation::ensemble(seed, spread, worlds);
    let mut per_cell = Vec::new();
    let mut robust = 0;
    let mut retained_total = 0.0;
    let mut retained_count = 0;

    for elite in report.archive.occupants() {
        let score = evaluate_ensemble(robot, &elite.genome, &ensemble, report.ticks);
        if score.falls == 0 {
            robust += 1;
        }
        if elite.score.fitness > 1e-3 {
            retained_total += (score.fitness / elite.score.fitness).min(2.0);
            retained_count += 1;
        }
        per_cell.push(HoldoutCell {
            cell: elite.cell,
            training_fitness: elite.score.fitness,
            holdout_fitness: score.fitness,
            holdout_falls: score.falls,
        });
    }

    HoldoutReport {
        tested: per_cell.len(),
        robust,
        retained_fraction: if retained_count == 0 {
            0.0
        } else {
            retained_total / retained_count as f32
        },
        per_cell,
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchConfig, holdout, run_search};
    use crate::{config::RobotConfig, perturb::Spread};

    /// Small enough to keep the debug suite quick. The full run that produced
    /// the committed archive is gated in `tests/evolved_archive.rs` instead.
    fn tiny() -> SearchConfig {
        SearchConfig {
            generations: 3,
            batch: 4,
            ticks: 30,
            worlds: 2,
            ..SearchConfig::default()
        }
    }

    #[test]
    fn a_search_fills_cells_and_records_its_history() {
        let report = run_search(&tiny(), &RobotConfig::nominal());

        assert_eq!(report.history.len(), 3);
        assert_eq!(report.evaluated, 12);
        assert!(report.archive.occupied_count() > 0);
        assert!(report.best().is_some());
    }

    #[test]
    fn the_archive_never_shrinks_across_generations() {
        // MAP-Elites only ever replaces a cell's occupant with a better one,
        // so coverage is monotonic. If it ever fell, insertion would be
        // evicting elites it should have kept.
        let report = run_search(&tiny(), &RobotConfig::nominal());

        for pair in report.history.windows(2) {
            assert!(
                pair[1].occupied >= pair[0].occupied,
                "coverage fell from {} to {}",
                pair[0].occupied,
                pair[1].occupied
            );
        }
    }

    #[test]
    fn a_search_is_reproducible_from_its_seed() {
        let config = tiny();
        let robot = RobotConfig::nominal();

        let first = run_search(&config, &robot);
        let second = run_search(&config, &robot);

        assert_eq!(first.archive, second.archive);
        assert_eq!(first.history, second.history);
    }

    #[test]
    fn different_seeds_explore_differently() {
        let robot = RobotConfig::nominal();
        let first = run_search(&tiny(), &robot);
        let second = run_search(&SearchConfig { seed: 99, ..tiny() }, &robot);

        assert_ne!(first.archive, second.archive);
    }

    #[test]
    fn a_report_round_trips_through_json() {
        let report = run_search(&tiny(), &RobotConfig::nominal());
        let decoded = super::SearchReport::from_json(&report.to_json()).expect("valid json");

        assert_eq!(decoded.archive, report.archive);
        assert_eq!(decoded.history, report.history);
    }

    #[test]
    fn every_archived_elite_stood_in_at_least_one_world() {
        // The archive is a repertoire of gaits. A controller that fell
        // everywhere is not a gait, and must never occupy a cell.
        let report = run_search(&tiny(), &RobotConfig::nominal());

        for elite in report.archive.occupants() {
            assert!(elite.score.falls < elite.score.worlds, "{:?}", elite.cell);
        }
    }

    #[test]
    fn the_holdout_scores_every_elite_on_unseen_worlds() {
        let report = run_search(&tiny(), &RobotConfig::nominal());
        let held = holdout(
            &report,
            &RobotConfig::nominal(),
            &Spread::default(),
            0xbeef,
            2,
        );

        assert_eq!(held.tested, report.archive.occupied_count());
        assert!(held.robust_fraction() <= 1.0);
        assert!(held.retained_fraction.is_finite());
    }
}
