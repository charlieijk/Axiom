use axiom::{
    Axis, EvolutionCheckpoint, EvolutionConfig, Genome, SearchMode, TaskKind,
    animation::{TerminalAnimationConfig, play_terminal_animation},
    evolution::run_evolution,
    fitness::evaluate,
    policy::ControllerKind,
    rng::Rng,
    save_checkpoint,
    web::{GuiConfig, gui_smoke_check, serve_gui},
};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("demo") => demo(),
        Some("evaluate") => evaluate_once(args.next().as_deref()),
        Some("evolve") => evolve(args.collect()),
        Some("animate") | Some("replay") => animate(args.collect()),
        Some("gui") | Some("serve") => gui(args.collect()),
        Some("help") | Some("--help") | Some("-h") | None => print_help(),
        Some(command) => {
            eprintln!("unknown command: {command}");
            print_help();
            std::process::exit(2);
        }
    }
}

fn demo() {
    let mut rng = Rng::new(11);
    for controller in [
        ControllerKind::FeedForward,
        ControllerKind::Recurrent,
        ControllerKind::Cpg,
    ] {
        let genome = axiom::Genome::minimal(controller, &mut rng);
        let evaluation = evaluate(&genome, TaskKind::RoughTerrain, 180);
        println!(
            "{:<11} fitness={:>7.3} distance={:>6.3} stable={:>6.3} upright={:>5.3}",
            controller.as_str(),
            evaluation.fitness,
            evaluation.metrics.distance,
            evaluation.metrics.stability,
            evaluation.metrics.uprightness
        );
    }
}

fn evaluate_once(controller: Option<&str>) {
    let kind = match parse_controller(controller.unwrap_or("cpg")) {
        Some(kind) => kind,
        None => {
            eprintln!("unknown controller kind: {}", controller.unwrap_or("cpg"));
            std::process::exit(2);
        }
    };
    let mut rng = Rng::new(19);
    let genome = Genome::minimal(kind, &mut rng);
    let evaluation = evaluate(&genome, TaskKind::RoughTerrain, 240);
    println!("controller: {}", kind.as_str());
    println!("fitness:    {:.3}", evaluation.fitness);
    println!("distance:   {:.3}", evaluation.metrics.distance);
    println!("stable dst: {:.3}", evaluation.metrics.stable_distance);
    println!("jump:       {:.3}", evaluation.metrics.jump_height);
    println!("upright:    {:.3}", evaluation.metrics.uprightness);
    println!("stability:  {:.3}", evaluation.metrics.stability);
    println!("end tilt:   {:.3}", evaluation.metrics.terminal_tilt);
    println!("body parts: {:.0}", evaluation.metrics.body_count);
    println!("actuators:  {:.0}", evaluation.metrics.actuator_count);
}

fn evolve(args: Vec<String>) {
    let mut config = EvolutionConfig::default();
    let mut checkpoint_path = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--generations" | "-g" => {
                config.generations = parse_next(&args, &mut index, "generations");
            }
            "--population" | "-p" => {
                config.population_size = parse_next(&args, &mut index, "population");
            }
            "--steps" => {
                config.evaluation_steps = parse_next(&args, &mut index, "steps");
            }
            "--task" => {
                let value: String = parse_next(&args, &mut index, "task");
                config.task = parse_task_or_exit(&value);
            }
            "--classic" => {
                config.search_mode = SearchMode::Classic;
            }
            "--seed" => {
                config.seed = parse_next(&args, &mut index, "seed");
            }
            "--archive-width" => {
                config.archive_width = parse_next(&args, &mut index, "archive width");
            }
            "--archive-height" => {
                config.archive_height = parse_next(&args, &mut index, "archive height");
            }
            "--x-axis" => {
                let value: String = parse_next(&args, &mut index, "x-axis");
                config.archive_x_axis = parse_axis_or_exit(&value);
            }
            "--y-axis" => {
                let value: String = parse_next(&args, &mut index, "y-axis");
                config.archive_y_axis = parse_axis_or_exit(&value);
            }
            "--checkpoint" => {
                checkpoint_path = Some(parse_next(&args, &mut index, "checkpoint"));
            }
            other => {
                eprintln!("unknown evolve option: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let report = run_evolution(config).unwrap_or_else(|error| {
        eprintln!("evolution failed: {error}");
        std::process::exit(2);
    });
    println!("generations: {}", report.generations);
    println!("best fitness: {:.3}", report.best_evaluation.fitness);
    println!(
        "best distance: {:.3}",
        report.best_evaluation.metrics.distance
    );
    println!(
        "best stable distance: {:.3}",
        report.best_evaluation.metrics.stable_distance
    );
    println!(
        "best stability: {:.3}",
        report.best_evaluation.metrics.stability
    );
    println!(
        "best controller: {}",
        report.best_genome.controller.as_str()
    );
    println!(
        "archive: {} occupied cells ({:.1}% coverage)",
        report.archive.occupied_count(),
        report.archive.coverage() * 100.0
    );

    if let Some(path) = checkpoint_path {
        let checkpoint = EvolutionCheckpoint::from_report(report);
        if let Err(error) = save_checkpoint(&path, &checkpoint) {
            eprintln!("checkpoint failed: {error}");
            std::process::exit(1);
        }
        println!("checkpoint: {path}");
    }
}

fn animate(args: Vec<String>) {
    let mut config = TerminalAnimationConfig::default();
    let mut controller = ControllerKind::Cpg;
    let mut task = TaskKind::RoughTerrain;
    let mut seed = 19_u64;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--frames" | "-f" => {
                config.steps = parse_next(&args, &mut index, "frames");
            }
            "--fps" => {
                config.fps = parse_next(&args, &mut index, "fps");
            }
            "--width" => {
                config.width = parse_next(&args, &mut index, "width");
            }
            "--height" => {
                config.height = parse_next(&args, &mut index, "height");
            }
            "--seed" => {
                seed = parse_next(&args, &mut index, "seed");
            }
            "--task" => {
                let value: String = parse_next(&args, &mut index, "task");
                task = parse_task_or_exit(&value);
            }
            "--no-clear" => {
                config.clear_screen = false;
            }
            value => {
                controller = match parse_controller(value) {
                    Some(controller) => controller,
                    None => {
                        eprintln!("unknown animate option or controller: {value}");
                        std::process::exit(2);
                    }
                };
            }
        }
        index += 1;
    }

    let mut rng = Rng::new(seed);
    let genome = Genome::minimal(controller, &mut rng);
    if let Err(error) = play_terminal_animation(&genome, task, config) {
        eprintln!("animation failed: {error}");
        std::process::exit(1);
    }
}

fn gui(args: Vec<String>) {
    let mut config = GuiConfig::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--host" => {
                config.host = parse_next(&args, &mut index, "host");
            }
            "--port" | "-p" => {
                config.port = parse_next(&args, &mut index, "port");
            }
            "--check" => {
                config.check = true;
            }
            "--checkpoint-dir" => {
                config.checkpoint_dir = parse_next(&args, &mut index, "checkpoint-dir");
            }
            other => {
                eprintln!("unknown gui option: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let result = if config.check {
        gui_smoke_check().map(|_| ())
    } else {
        serve_gui(config).map(|_| ())
    };

    if let Err(error) = result {
        eprintln!("gui failed: {error}");
        std::process::exit(1);
    }
}

fn parse_next<T: std::str::FromStr>(args: &[String], index: &mut usize, label: &str) -> T {
    *index += 1;
    args.get(*index)
        .unwrap_or_else(|| {
            eprintln!("missing value for {label}");
            std::process::exit(2);
        })
        .parse()
        .unwrap_or_else(|_| {
            eprintln!("invalid value for {label}");
            std::process::exit(2);
        })
}

fn parse_controller(value: &str) -> Option<ControllerKind> {
    ControllerKind::parse(value)
}

fn parse_task(value: &str) -> Option<TaskKind> {
    TaskKind::parse(value)
}

fn parse_task_or_exit(value: &str) -> TaskKind {
    parse_task(value).unwrap_or_else(|| {
        eprintln!("unknown task: {value}");
        std::process::exit(2);
    })
}

fn parse_axis_or_exit(value: &str) -> Axis {
    Axis::parse(value).unwrap_or_else(|| {
        eprintln!("unknown archive axis: {value}");
        std::process::exit(2);
    })
}

fn print_help() {
    println!(
        "Axiom embodied evolution\n\n\
         Commands:\n\
           axiom demo\n\
           axiom evaluate [feedforward|recurrent|cpg]\n\
           axiom animate [feedforward|recurrent|cpg] [--task flat|rough|recovery] [--frames N] [--fps N]\n\
           axiom gui [--host HOST] [--port PORT] [--checkpoint-dir DIR] [--check]\n\
           axiom evolve [--task flat|rough|recovery] [--generations N] [--population N] [--steps N]\n\
                        [--archive-width N] [--archive-height N] [--x-axis AXIS] [--y-axis AXIS]\n\
                        [--classic] [--seed N] [--checkpoint PATH]\n"
    );
}
