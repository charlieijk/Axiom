use axiom::{
    EvolutionCheckpoint, EvolutionConfig, Genome, SearchMode, TaskKind, TaskPackKind,
    animation::{TerminalAnimationConfig, play_terminal_animation},
    checkpoint::{load_checkpoint, save_checkpoint},
    evolution::{run_evolution, run_evolution_with_pack},
    fitness::evaluate,
    policy::ControllerKind,
    rng::Rng,
    save_report_json, save_report_markdown,
    web::{GuiConfig, gui_smoke_check, serve_gui},
};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("demo") => demo(),
        Some("evaluate") => evaluate_once(args.next().as_deref()),
        Some("evolve") => evolve(args.collect()),
        Some("animate") | Some("replay") => animate(args.collect()),
        Some("inspect") | Some("checkpoint") => inspect_checkpoint(args.collect()),
        Some("gui") | Some("serve") => gui(args.collect()),
        Some("--version") | Some("-V") => println!("axiom {}", env!("CARGO_PKG_VERSION")),
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
    let mut task_pack = None;
    let mut save_path = None;
    let mut report_json_path = None;
    let mut report_markdown_path = None;
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
                config.task = parse_task(&value).unwrap_or_else(|| {
                    eprintln!("unknown task: {value}");
                    std::process::exit(2);
                });
                task_pack = None;
            }
            "--pack" => {
                let value: String = parse_next(&args, &mut index, "pack");
                task_pack = Some(TaskPackKind::parse(&value).unwrap_or_else(|| {
                    eprintln!("unknown task pack: {value}");
                    std::process::exit(2);
                }));
            }
            "--classic" => {
                config.search_mode = SearchMode::Classic;
            }
            "--seed" => {
                config.seed = parse_next(&args, &mut index, "seed");
            }
            "--save" | "--checkpoint" => {
                save_path = Some(parse_next::<String>(&args, &mut index, "checkpoint path"));
            }
            "--report-json" => {
                report_json_path =
                    Some(parse_next::<String>(&args, &mut index, "JSON report path"));
            }
            "--report-md" => {
                report_markdown_path = Some(parse_next::<String>(
                    &args,
                    &mut index,
                    "Markdown report path",
                ));
            }
            other => {
                eprintln!("unknown evolve option: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let report = match task_pack {
        Some(pack) => run_evolution_with_pack(config.clone(), pack),
        None => run_evolution(config.clone()),
    }
    .unwrap_or_else(|error| {
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
    println!("lineage records: {}", report.lineage.len());

    if let Some(path) = report_json_path {
        save_report_json(&path, &config, task_pack, &report).unwrap_or_else(|error| {
            eprintln!("failed to save JSON report {path}: {error}");
            std::process::exit(1);
        });
        println!("saved JSON report: {path}");
    }

    if let Some(path) = report_markdown_path {
        save_report_markdown(&path, &config, task_pack, &report).unwrap_or_else(|error| {
            eprintln!("failed to save Markdown report {path}: {error}");
            std::process::exit(1);
        });
        println!("saved Markdown report: {path}");
    }

    if let Some(path) = save_path {
        let checkpoint = EvolutionCheckpoint::from_report(config, report);
        save_checkpoint(&path, &checkpoint).unwrap_or_else(|error| {
            eprintln!("failed to save checkpoint {path}: {error}");
            std::process::exit(1);
        });
        println!("saved checkpoint: {path}");
    }
}

fn animate(args: Vec<String>) {
    let mut config = TerminalAnimationConfig::default();
    let mut controller = None;
    let mut task = TaskKind::RoughTerrain;
    let mut task_explicit = false;
    let mut checkpoint_path = None;
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
                task = match parse_task(&value) {
                    Some(task) => task,
                    None => {
                        eprintln!("unknown task: {value}");
                        std::process::exit(2);
                    }
                };
                task_explicit = true;
            }
            "--checkpoint" => {
                checkpoint_path = Some(parse_next::<String>(&args, &mut index, "checkpoint path"));
            }
            "--no-clear" => {
                config.clear_screen = false;
            }
            value => {
                controller = match parse_controller(value) {
                    Some(controller) => Some(controller),
                    None => {
                        eprintln!("unknown animate option or controller: {value}");
                        std::process::exit(2);
                    }
                };
            }
        }
        index += 1;
    }

    let genome = if let Some(path) = checkpoint_path {
        let checkpoint = load_checkpoint(&path).unwrap_or_else(|error| {
            eprintln!("failed to load checkpoint {path}: {error}");
            std::process::exit(1);
        });
        if !task_explicit {
            task = checkpoint.config.task;
        }
        checkpoint.report.best_genome
    } else {
        let mut rng = Rng::new(seed);
        Genome::minimal(controller.unwrap_or(ControllerKind::Cpg), &mut rng)
    };
    if let Err(error) = play_terminal_animation(&genome, task, config) {
        eprintln!("animation failed: {error}");
        std::process::exit(1);
    }
}

fn inspect_checkpoint(args: Vec<String>) {
    let Some(path) = args.first() else {
        eprintln!("missing checkpoint path");
        std::process::exit(2);
    };
    if args.len() > 1 {
        eprintln!("inspect accepts exactly one checkpoint path");
        std::process::exit(2);
    }

    let checkpoint = load_checkpoint(path).unwrap_or_else(|error| {
        eprintln!("failed to load checkpoint {path}: {error}");
        std::process::exit(1);
    });
    println!("checkpoint version: {}", checkpoint.version);
    println!("seed:               {}", checkpoint.config.seed);
    println!("generations:        {}", checkpoint.report.generations);
    println!("evaluated genomes:  {}", checkpoint.report.evaluated_count);
    println!("lineage records:    {}", checkpoint.report.lineage.len());
    println!("best genome id:     {}", checkpoint.report.best_genome_id);
    println!(
        "best fitness:       {:.3}",
        checkpoint.report.best_evaluation.fitness
    );
    println!(
        "archive:            {} occupied cells ({:.1}% coverage)",
        checkpoint.report.archive.occupied_count(),
        checkpoint.report.archive.coverage() * 100.0
    );
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
    match value {
        "feedforward" | "ff" => Some(ControllerKind::FeedForward),
        "recurrent" | "rnn" => Some(ControllerKind::Recurrent),
        "cpg" => Some(ControllerKind::Cpg),
        _ => None,
    }
}

fn parse_task(value: &str) -> Option<TaskKind> {
    match value {
        "flat" | "flat-run" => Some(TaskKind::FlatRun),
        "rough" | "rough-terrain" => Some(TaskKind::RoughTerrain),
        "recovery" => Some(TaskKind::Recovery),
        _ => None,
    }
}

fn print_help() {
    println!(
        "Axiom embodied evolution\n\n\
         Commands:\n\
           axiom demo\n\
           axiom evaluate [feedforward|recurrent|cpg]\n\
           axiom animate [feedforward|recurrent|cpg] [--task flat|rough|recovery] [--frames N] [--fps N]\n\
           axiom animate --checkpoint PATH [--task flat|rough|recovery] [--frames N] [--fps N]\n\
           axiom inspect PATH\n\
           axiom gui [--host HOST] [--port PORT] [--check]\n\
           axiom evolve [--task flat|rough|recovery] [--pack rough-inspection]\n\
                        [--generations N] [--population N] [--steps N] [--classic] [--seed N]\n\
                        [--save PATH] [--report-json PATH] [--report-md PATH]\n"
    );
}
