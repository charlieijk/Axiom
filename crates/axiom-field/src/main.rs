//! Field lab CLI: stand the robot up, run the reference gait, and record or
//! verify trajectories.

use std::process::ExitCode;

use axiom_field::{RobotConfig, Trajectory, gait::Gait, sim::FieldSim};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments.first().map(String::as_str);
    let rest = arguments.get(1..).unwrap_or_default();

    match command {
        Some("stand") => stand(rest),
        Some("gait") => gait(rest),
        Some("record") => record(rest),
        Some("verify") => verify(rest),
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
  stand   [--config <robot.toml>] [--ticks <n>]
          Settle the robot from its spawn drop and report the resting pose.

  gait    [--config <robot.toml>] [--ticks <n>]
          [--freq <hz>] [--hip <a>] [--knee <a>] [--knee-phase <cycles>]
          Run the reference trot and report what it achieved.

  record  --out <trajectory.json> [--config <robot.toml>] [--ticks <n>] [--stride <n>]
          Record the reference trot as a golden trajectory.

  verify  <trajectory.json> [--config <robot.toml>]
          Replay the reference trot and compare it against a golden file.

All physical parameters live in robot.toml. Calibrating this model against a
real robot is a diff of that file, never a code change."
    );
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
