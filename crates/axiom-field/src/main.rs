//! Field lab CLI: stand the robot up, run the reference gait, and record or
//! verify trajectories.

use std::process::ExitCode;

use axiom::{qd::AxisSpec, rng::Rng};
use axiom_field::{
    RobotConfig, Trajectory,
    controller::CpgGenome,
    evaluate::evaluate_ensemble,
    gait::{Gait, sine_gait},
    perturb::{Perturbation, Spread},
    search::{SearchConfig, SearchReport, holdout, run_search},
    sim::FieldSim,
};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);
    let rest = arguments.get(1..).unwrap_or_default();

    match command {
        Some("lab") => lab(rest),
        Some("stand") => stand(rest),
        Some("gait") => gait(rest),
        Some("record") => record(rest),
        Some("verify") => verify(rest),
        Some("pilot") => pilot(rest),
        Some("evolve") => evolve(rest),
        Some("holdout") => holdout_command(rest),
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "axiom-field — rigid-body field lab for a fixed hobby-servo quadruped

USAGE:
  lab     [--port <n>]
          Open the local playable 3D rigid-body test range on port 8790, or
          the next free port when that one is held; --port picks another.

  stand   [--config <robot.toml>] [--ticks <n>]
          Settle the robot from its spawn drop and report the resting pose.

  gait    [--config <robot.toml>] [--ticks <n>]
          [--freq <hz>] [--hip <a>] [--knee <a>] [--knee-phase <cycles>]
          Run the reference trot and report what it achieved.

  record  --out <trajectory.json> [--config <robot.toml>] [--ticks <n>] [--stride <n>]
          Record the reference trot as a golden trajectory.

  verify  <trajectory.json> [--config <robot.toml>]
          Replay the reference trot and compare it against a golden file.

  pilot   [--config <robot.toml>] [--samples <n>] [--ticks <n>] [--seed <n>]
          Sample random and mutated controllers and report the spread of the
          behaviour descriptors. Run this before choosing archive axis ranges:
          a guessed range leaves most of the archive unreachable.

  evolve  [--config <robot.toml>] [--seed <n>] [--generations <n>] [--batch <n>]
          [--ticks <n>] [--worlds <n>] [--out <report.json>]
          Run MAP-Elites over the simulator with domain randomization.

  holdout <report.json> [--config <robot.toml>] [--seed <n>] [--worlds <n>]
          Re-score an archive on worlds the search never trained on.

All physical parameters live in robot.toml. Calibrating this model against a
real robot is a diff of that file, never a code change."
    );
}

fn lab(arguments: &[String]) -> ExitCode {
    let mut port = axiom_field::lab::DEFAULT_PORT;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--port" | "-p" => {
                index += 1;
                match arguments
                    .get(index)
                    .and_then(|value| value.parse::<u16>().ok())
                {
                    Some(value) => port = value,
                    None => {
                        eprintln!("lab: --port needs a number from 0 to 65535");
                        return ExitCode::from(2);
                    }
                }
            }
            other => {
                eprintln!("unknown lab option: {other}");
                print_help();
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    match axiom_field::lab::serve(port) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("field lab: {error}");
            ExitCode::FAILURE
        }
    }
}

fn load(arguments: &[String]) -> Result<RobotConfig, ExitCode> {
    match flag(arguments, "--config") {
        Some(path) => RobotConfig::load(&path).map_err(|error| {
            eprintln!("{error}");
            ExitCode::from(2)
        }),
        None => Ok(RobotConfig::nominal()),
    }
}

fn flag(arguments: &[String], name: &str) -> Option<String> {
    let index = arguments.iter().position(|argument| argument == name)?;
    arguments.get(index + 1).cloned()
}

fn number(arguments: &[String], name: &str, fallback: usize) -> usize {
    flag(arguments, name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn decimal(arguments: &[String], name: &str, fallback: f32) -> f32 {
    flag(arguments, name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

/// Gait parameters are flags so that exploring them never requires editing
/// source — which is how the committed defaults were chosen.
fn gait_from(arguments: &[String]) -> Gait {
    let default = Gait::default();
    Gait {
        frequency_hz: decimal(arguments, "--freq", default.frequency_hz),
        hip_amplitude: decimal(arguments, "--hip", default.hip_amplitude),
        knee_amplitude: decimal(arguments, "--knee", default.knee_amplitude),
        knee_phase_offset: decimal(arguments, "--knee-phase", default.knee_phase_offset),
    }
}

fn report_provenance(config: &RobotConfig) {
    println!("robot:      {}", config.meta.name);
    if !config.meta.calibrated {
        println!(
            "WARNING:    these parameters are UNCALIBRATED ({}).",
            config.meta.provenance
        );
        println!("            Numbers below describe a robot nobody has built yet.");
    }
}

fn stand(arguments: &[String]) -> ExitCode {
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let ticks = number(arguments, "--ticks", 200);

    report_provenance(&config);
    println!("standing:   {:.4} m (derived)", config.standing_height_m());
    println!("spawn:      {:.4} m", config.spawn_height_m());

    let spawn = config.spawn_height_m();
    let mut sim = FieldSim::new(config);
    let settled = sim.settle(ticks);

    println!("rest:       {:.4} m", settled.height_m());
    println!(
        "sag:        {:.4} m under its own weight",
        spawn - settled.height_m()
    );
    println!(
        "drift:      {:+.4} m forward, {:+.4} m lateral",
        settled.forward_m(),
        settled.lateral_m()
    );
    println!("tilt:       {:.4} rad", settled.tilt_rad);
    ExitCode::SUCCESS
}

fn gait(arguments: &[String]) -> ExitCode {
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let ticks = number(arguments, "--ticks", 200);

    report_provenance(&config);
    let control_hz = config.sim.control_hz;
    let gait = gait_from(arguments);
    let standing = config.standing_height_m();
    let mut sim = FieldSim::new(config);

    // Let the drop settle before the gait starts, so travel measures walking
    // rather than the robot falling the last few millimetres onto its feet.
    let settled = sim.settle(100);
    let start = settled.clone();
    let mut last = settled;
    for tick in 0..ticks {
        last = sim.control_step(&gait.actions_at(tick as f32 / control_hz));
    }

    let seconds = ticks as f32 / control_hz;
    let travel = last.forward_m() - start.forward_m();
    println!(
        "gait:       trot at {:.2} Hz for {seconds:.1} s",
        gait.frequency_hz
    );
    println!("travel:     {travel:+.4} m forward");
    println!("speed:      {:+.4} m/s", travel / seconds);
    println!("lateral:    {:+.4} m", last.lateral_m() - start.lateral_m());
    println!(
        "heading:    {:+.4} rad",
        last.heading_rad - start.heading_rad
    );
    println!("tilt:       {:.4} rad", last.tilt_rad);
    println!(
        "height:     {:.4} m of {standing:.4} m standing",
        last.height_m()
    );
    // Travel produced by falling over is not locomotion. Say so rather than
    // reporting a distance that looks like walking.
    if last.height_m() < standing * 0.6 || last.tilt_rad > 0.5 {
        println!("UPRIGHT:    NO — the robot fell; this travel is not walking");
    } else {
        println!("UPRIGHT:    yes");
    }
    ExitCode::SUCCESS
}

fn record(arguments: &[String]) -> ExitCode {
    let Some(out) = flag(arguments, "--out") else {
        eprintln!("record requires --out <trajectory.json>");
        return ExitCode::from(2);
    };
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let ticks = number(arguments, "--ticks", 200);
    let stride = number(arguments, "--stride", 10);

    report_provenance(&config);
    let trajectory = Trajectory::record(config, gait_from(arguments), ticks, stride);
    match std::fs::write(&out, trajectory.to_json()) {
        Ok(()) => {
            println!(
                "recorded:   {} samples of {ticks} ticks -> {out}",
                trajectory.samples.len()
            );
            println!("travel:     {:+.4} m", trajectory.forward_travel_m());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write {out}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn verify(arguments: &[String]) -> ExitCode {
    let Some(path) = arguments.first().filter(|value| !value.starts_with("--")) else {
        eprintln!("verify requires a golden trajectory path");
        return ExitCode::from(2);
    };
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("failed to read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let golden = match Trajectory::from_json(&text) {
        Ok(golden) => golden,
        Err(error) => {
            eprintln!("failed to parse {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let replay = Trajectory::record(config, gait_from(arguments), golden.ticks, golden.stride);
    match replay.compare(&golden) {
        Ok(()) => {
            println!("verified:   {} samples match {path}", replay.samples.len());
            println!("travel:     {:+.4} m", replay.forward_travel_m());
            ExitCode::SUCCESS
        }
        Err(divergence) => {
            eprintln!("trajectory diverged from {path}");
            eprintln!("  {divergence}");
            ExitCode::FAILURE
        }
    }
}

fn pilot(arguments: &[String]) -> ExitCode {
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let samples = number(arguments, "--samples", 120);
    let ticks = number(arguments, "--ticks", 160);
    let seed = number(arguments, "--seed", 7) as u64;

    report_provenance(&config);
    let spread = Spread::default();
    let ensemble = Perturbation::ensemble(seed ^ 0x5eed, &spread, 3);
    let mut rng = Rng::new(seed);
    let seed_genome = CpgGenome::from_gait(&sine_gait());

    let mut speeds = Vec::new();
    let mut efforts = Vec::new();
    let mut standing = 0usize;

    for index in 0..samples {
        let genome = if index % 2 == 0 {
            seed_genome.mutate(&mut rng, 0.25)
        } else {
            CpgGenome::random(&mut rng)
        };
        let score = evaluate_ensemble(&config, &genome, &ensemble, ticks);
        if score.falls == score.worlds {
            continue;
        }
        standing += 1;
        speeds.push(score.mean_speed_m_s);
        efforts.push(score.mean_effort_per_m);
    }

    if speeds.is_empty() {
        eprintln!("no sampled controller stayed upright; nothing to calibrate from");
        return ExitCode::FAILURE;
    }

    println!("samples:    {samples} ({standing} stood in at least one world)");
    print_percentiles("speed (m/s)", &mut speeds);
    print_percentiles("effort (rad/m)", &mut efforts);
    let mut log_efforts: Vec<f32> = efforts.iter().map(|e| e.max(1e-3).log10()).collect();
    // The archive bins economy on a log scale, so calibrate on these.
    print_percentiles("log10 effort", &mut log_efforts);
    println!();
    println!("Set the archive axes to cover roughly p05..p95 of these.");
    ExitCode::SUCCESS
}

fn print_percentiles(label: &str, values: &mut [f32]) {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let at = |q: f32| -> f32 {
        let index = ((values.len() - 1) as f32 * q).round() as usize;
        values[index]
    };
    println!(
        "{label:>16}: min {:>9.3}  p05 {:>9.3}  p50 {:>9.3}  p95 {:>9.3}  max {:>9.3}",
        values[0],
        at(0.05),
        at(0.50),
        at(0.95),
        values[values.len() - 1]
    );
}

fn search_config_from(arguments: &[String]) -> SearchConfig {
    let default = SearchConfig::default();
    SearchConfig {
        seed: number(arguments, "--seed", default.seed as usize) as u64,
        generations: number(arguments, "--generations", default.generations),
        batch: number(arguments, "--batch", default.batch),
        ticks: number(arguments, "--ticks", default.ticks),
        worlds: number(arguments, "--worlds", default.worlds),
        speed_axis: AxisSpec::new(
            default.speed_axis.label.clone(),
            default.speed_axis.min,
            decimal(arguments, "--max-speed", default.speed_axis.max),
        ),
        effort_axis: AxisSpec::new(
            default.effort_axis.label.clone(),
            default.effort_axis.min,
            decimal(arguments, "--max-effort", default.effort_axis.max),
        ),
        ..default
    }
}

fn evolve(arguments: &[String]) -> ExitCode {
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let search = search_config_from(arguments);

    report_provenance(&config);
    println!(
        "search:     {} generations x {} genomes x {} worlds",
        search.generations, search.batch, search.worlds
    );
    // The reference trot scored on the very same worlds, so "better than the
    // hand-tuned gait" is a measurement rather than a claim.
    let baseline = evaluate_ensemble(
        &config,
        &CpgGenome::from_gait(&sine_gait()),
        &search.training_ensemble(),
        search.ticks,
    );
    let report = run_search(&search, &config);

    println!(
        "reference:  trot walks {:.4} m/s on these worlds (fell in {} of {})",
        baseline.mean_speed_m_s, baseline.falls, baseline.worlds
    );
    println!(
        "evaluated:  {} genomes ({} simulated runs)",
        report.evaluated,
        report.evaluated * report.worlds
    );
    println!(
        "archive:    {} of {} cells ({:.1}% coverage)",
        report.archive.occupied_count(),
        search.archive_width * search.archive_height,
        report.coverage() * 100.0
    );
    match report.best() {
        Some(best) => {
            println!(
                "best:       fitness {:.4} m, speed {:.4} m/s, cell {:?}",
                best.score.fitness, best.score.mean_speed_m_s, best.cell
            );
            println!(
                "            worst world {:.4} m, fell in {} of {} worlds",
                best.score.worst_fitness, best.score.falls, best.score.worlds
            );
            if baseline.mean_speed_m_s.abs() > 1e-4 {
                println!(
                    "            {:+.0}% speed against the reference trot",
                    (best.score.mean_speed_m_s / baseline.mean_speed_m_s - 1.0) * 100.0
                );
            }
        }
        None => println!("best:       none — no controller stayed upright"),
    }

    if let Some(out) = flag(arguments, "--out") {
        if let Err(error) = std::fs::write(&out, report.to_json()) {
            eprintln!("failed to write {out}: {error}");
            return ExitCode::FAILURE;
        }
        println!("saved:      {out}");
    }
    ExitCode::SUCCESS
}

fn holdout_command(arguments: &[String]) -> ExitCode {
    let Some(path) = arguments.first().filter(|value| !value.starts_with("--")) else {
        eprintln!("holdout requires a search report path");
        return ExitCode::from(2);
    };
    let config = match load(arguments) {
        Ok(config) => config,
        Err(code) => return code,
    };
    let seed = number(arguments, "--seed", 0xbeef) as u64;
    let worlds = number(arguments, "--worlds", 6);

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("failed to read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let report = match SearchReport::from_json(&text) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("failed to parse {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let held = holdout(&report, &config, &Spread::default(), seed, worlds);
    println!("elites:     {}", held.tested);
    println!(
        "robust:     {} stood in all {worlds} unseen worlds ({:.0}%)",
        held.robust,
        held.robust_fraction() * 100.0
    );
    println!(
        "retained:   {:.0}% of training fitness on unseen worlds",
        held.retained_fraction * 100.0
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{flag, number};

    #[test]
    fn flags_read_their_following_value() {
        let arguments = vec!["--ticks".to_string(), "40".to_string()];

        assert_eq!(flag(&arguments, "--ticks"), Some("40".to_string()));
        assert_eq!(number(&arguments, "--ticks", 7), 40);
    }

    #[test]
    fn a_missing_or_malformed_flag_falls_back() {
        let arguments = vec!["--ticks".to_string(), "not-a-number".to_string()];

        assert_eq!(number(&arguments, "--ticks", 7), 7);
        assert_eq!(number(&arguments, "--stride", 3), 3);
        assert_eq!(flag(&arguments, "--stride"), None);
    }

    #[test]
    fn a_trailing_flag_without_a_value_does_not_panic() {
        let arguments = vec!["--out".to_string()];

        assert_eq!(flag(&arguments, "--out"), None);
    }

    #[test]
    fn the_actuator_count_is_the_documented_eight() {
        assert_eq!(axiom_field::sim::ACTUATOR_COUNT, 8);
    }
}
