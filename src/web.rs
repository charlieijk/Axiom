use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
};

use serde::Serialize;

use crate::{
    EvolutionConfig, Genome, SearchMode, TaskKind, animation::capture_replay, evaluate,
    policy::ControllerKind, rng::Rng, run_evolution,
};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");
const GRAPHICS_HTML: &str = include_str!("../web/graphics3d.html");
const GRAPHICS_CSS: &str = include_str!("../web/graphics3d.css");
const GRAPHICS_JS: &str = include_str!("../web/graphics3d.js");
// `/api/replay?mode=evolved` is an interactive preview, not a full experiment runner.
// Defaults stay around 60k simulated evaluation steps; longer searches belong in the CLI.
const MAX_EVOLVED_REPLAY_COST: usize = 250_000;

#[derive(Clone, Debug)]
pub struct GuiConfig {
    pub host: String,
    pub port: u16,
    pub check: bool,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8787,
            check: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ReplayRequest {
    mode: ReplayMode,
    controller: ControllerKind,
    task: TaskKind,
    seed: u64,
    frames: usize,
    dt: f32,
    generations: usize,
    population_size: usize,
    evaluation_steps: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplayMode {
    Minimal,
    Evolved,
}

impl ReplayMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Evolved => "evolved",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ReplayRequestError {
    EvolvedBudgetExceeded { cost: usize, max_cost: usize },
}

#[derive(Serialize)]
struct ReplayErrorResponse {
    error: &'static str,
    cost: usize,
    max_cost: usize,
}

#[derive(Serialize)]
struct ReplayResponse {
    controller: &'static str,
    task: &'static str,
    seed: u64,
    source: &'static str,
    generations: usize,
    population: usize,
    evaluation_steps: usize,
    fitness: f32,
    best_distance: f32,
    stable_distance: f32,
    uprightness: f32,
    stability: f32,
    terminal_tilt: f32,
    dt: f32,
    body: Vec<ReplayBodyNode>,
    frames: Vec<ReplayFrameResponse>,
}

#[derive(Serialize)]
struct ReplayBodyNode {
    id: usize,
    parent: Option<usize>,
    size: [f32; 2],
    actuator: f32,
}

#[derive(Serialize)]
struct ReplayFrameResponse {
    time: f32,
    root: [f32; 2],
    tilt: f32,
    bodies: Vec<[f32; 2]>,
    joints: Vec<[[f32; 2]; 2]>,
}

impl Default for ReplayRequest {
    fn default() -> Self {
        Self {
            mode: ReplayMode::Minimal,
            controller: ControllerKind::Cpg,
            task: TaskKind::RoughTerrain,
            seed: 19,
            frames: 220,
            dt: 0.05,
            generations: 12,
            population_size: 28,
            evaluation_steps: 180,
        }
    }
}

pub fn serve_gui(config: GuiConfig) -> io::Result<SocketAddr> {
    let listener = bind_first_available(&config.host, config.port)?;
    let address = listener.local_addr()?;
    println!("Axiom GUI running at http://{address}");
    println!("Press Ctrl-C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                let worker = thread::Builder::new()
                    .name("axiom-gui-request".to_string())
                    .spawn(move || {
                        if let Err(error) = handle_connection(stream) {
                            match peer {
                                Some(peer) => eprintln!("request from {peer} failed: {error}"),
                                None => eprintln!("request failed: {error}"),
                            }
                        }
                    });

                if let Err(error) = worker {
                    eprintln!("failed to start request worker: {error}");
                }
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(address)
}

pub fn gui_smoke_check() -> io::Result<()> {
    let html = response_body("/");
    let css = response_body("/styles.css");
    let js = response_body("/app.js");
    let graphics = response_body("/3d");
    let graphics_js = response_body("/graphics3d.js");
    let replay = replay_json(ReplayRequest {
        frames: 3,
        ..ReplayRequest::default()
    });

    if !html.contains("<canvas") || !css.contains(".simulator-shell") || !js.contains("fetchReplay")
    {
        return Err(io::Error::other(
            "GUI assets did not include expected UI markers",
        ));
    }
    if !graphics.contains("graphics-canvas") || !graphics_js.contains("THREE_MODULE_URL") {
        return Err(io::Error::other(
            "3D GUI assets did not include expected scene markers",
        ));
    }
    if !replay.contains("\"frames\"") || !replay.contains("\"bodies\"") {
        return Err(io::Error::other(
            "replay JSON did not include frame/body data",
        ));
    }

    println!("GUI smoke check passed");
    Ok(())
}

fn bind_first_available(host: &str, preferred_port: u16) -> io::Result<TcpListener> {
    let mut last_error = None;
    for offset in 0..20 {
        let Some(port) = preferred_port.checked_add(offset) else {
            break;
        };
        let address = format!("{host}:{port}");
        match TcpListener::bind(&address) {
            Ok(listener) => return Ok(listener),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| io::Error::other("no bind attempts were made")))
}

fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, content_type, body) = match path {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", response_body("/")),
        "/3d" | "/3d.html" => ("200 OK", "text/html; charset=utf-8", response_body("/3d")),
        "/styles.css" => ("200 OK", "text/css; charset=utf-8", response_body(path)),
        "/graphics3d.css" => ("200 OK", "text/css; charset=utf-8", response_body(path)),
        "/app.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
        ),
        "/graphics3d.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
        ),
        "/api/replay" => api_replay_response(query),
        _ => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found".to_string(),
        ),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())
}

fn response_body(path: &str) -> String {
    match path {
        "/" | "/index.html" => INDEX_HTML.to_string(),
        "/3d" | "/3d.html" => GRAPHICS_HTML.to_string(),
        "/styles.css" => APP_CSS.to_string(),
        "/app.js" => APP_JS.to_string(),
        "/graphics3d.css" => GRAPHICS_CSS.to_string(),
        "/graphics3d.js" => GRAPHICS_JS.to_string(),
        _ => String::new(),
    }
}

fn parse_replay_request(query: &str) -> ReplayRequest {
    let mut request = ReplayRequest::default();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = url_decode(value);
        match key {
            "mode" | "source" => {
                if let Some(mode) = parse_replay_mode(&value) {
                    request.mode = mode;
                }
            }
            "controller" => {
                if let Some(controller) = parse_controller(&value) {
                    request.controller = controller;
                }
            }
            "task" => {
                if let Some(task) = parse_task(&value) {
                    request.task = task;
                }
            }
            "seed" => {
                if let Ok(seed) = value.parse() {
                    request.seed = seed;
                }
            }
            "frames" => {
                if let Ok(frames) = value.parse::<usize>() {
                    request.frames = frames.clamp(1, 900);
                }
            }
            "generations" => {
                if let Ok(generations) = value.parse::<usize>() {
                    request.generations = generations.clamp(1, 40);
                }
            }
            "population" | "population_size" => {
                if let Ok(population_size) = value.parse::<usize>() {
                    request.population_size = population_size.clamp(4, 96);
                }
            }
            "evaluation_steps" | "evolve_steps" => {
                if let Ok(evaluation_steps) = value.parse::<usize>() {
                    request.evaluation_steps = evaluation_steps.clamp(20, 500);
                }
            }
            _ => {}
        }
    }

    request
}

fn api_replay_response(query: &str) -> (&'static str, &'static str, String) {
    let request = parse_replay_request(query);
    match validate_replay_request(&request) {
        Ok(()) => (
            "200 OK",
            "application/json; charset=utf-8",
            replay_json(request),
        ),
        Err(error) => (
            "400 Bad Request",
            "application/json; charset=utf-8",
            replay_error_json(error),
        ),
    }
}

fn validate_replay_request(request: &ReplayRequest) -> Result<(), ReplayRequestError> {
    if request.mode == ReplayMode::Evolved {
        let cost = evolved_replay_cost(request);
        if cost > MAX_EVOLVED_REPLAY_COST {
            return Err(ReplayRequestError::EvolvedBudgetExceeded {
                cost,
                max_cost: MAX_EVOLVED_REPLAY_COST,
            });
        }
    }

    Ok(())
}

fn evolved_replay_cost(request: &ReplayRequest) -> usize {
    request
        .generations
        .saturating_mul(request.population_size)
        .saturating_mul(request.evaluation_steps)
}

fn replay_error_json(error: ReplayRequestError) -> String {
    match error {
        ReplayRequestError::EvolvedBudgetExceeded { cost, max_cost } => {
            let response = ReplayErrorResponse {
                error: "evolved replay request exceeds the interactive preview budget",
                cost,
                max_cost,
            };
            serde_json::to_string(&response).expect("serializing replay error cannot fail")
        }
    }
}

fn replay_json(request: ReplayRequest) -> String {
    let (genome, evaluation, generations, population_size) = match request.mode {
        ReplayMode::Minimal => {
            let mut rng = Rng::new(request.seed);
            let genome = Genome::minimal(request.controller, &mut rng);
            let evaluation = evaluate(&genome, request.task, request.evaluation_steps);
            (genome, evaluation, 0, 1)
        }
        ReplayMode::Evolved => {
            let report = run_evolution(EvolutionConfig {
                seed: request.seed,
                population_size: request.population_size,
                generations: request.generations,
                evaluation_steps: request.evaluation_steps,
                task: request.task,
                search_mode: SearchMode::MapElites,
                ..EvolutionConfig::default()
            });
            (
                report.best_genome,
                report.best_evaluation,
                request.generations,
                request.population_size,
            )
        }
    };
    let frames = capture_replay(&genome, request.task, request.frames, request.dt);
    let response = ReplayResponse {
        controller: genome.controller.as_str(),
        task: task_name(request.task),
        seed: request.seed,
        source: request.mode.as_str(),
        generations,
        population: population_size,
        evaluation_steps: request.evaluation_steps,
        fitness: evaluation.fitness,
        best_distance: evaluation.metrics.distance,
        stable_distance: evaluation.metrics.stable_distance,
        uprightness: evaluation.metrics.uprightness,
        stability: evaluation.metrics.stability,
        terminal_tilt: evaluation.metrics.terminal_tilt,
        dt: request.dt,
        body: genome
            .body
            .nodes
            .iter()
            .map(|node| ReplayBodyNode {
                id: node.id,
                parent: node.parent,
                size: [node.size.x, node.size.y],
                actuator: node.actuator_strength,
            })
            .collect(),
        frames: frames
            .iter()
            .map(|frame| ReplayFrameResponse {
                time: frame.time,
                root: [frame.root_position.x, frame.root_position.y],
                tilt: frame.tilt,
                bodies: frame
                    .body_centers
                    .iter()
                    .map(|center| [center.x, center.y])
                    .collect(),
                joints: frame
                    .joint_segments
                    .iter()
                    .map(|(start, end)| [[start.x, start.y], [end.x, end.y]])
                    .collect(),
            })
            .collect(),
    };

    serde_json::to_string(&response).expect("serializing replay response cannot fail")
}

fn parse_replay_mode(value: &str) -> Option<ReplayMode> {
    match value {
        "minimal" | "seed" | "raw" => Some(ReplayMode::Minimal),
        "evolved" | "evolve" | "champion" => Some(ReplayMode::Evolved),
        _ => None,
    }
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

fn task_name(task: TaskKind) -> &'static str {
    match task {
        TaskKind::FlatRun => "flat",
        TaskKind::RoughTerrain => "rough",
        TaskKind::Recovery => "recovery",
    }
}

fn url_decode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        match byte {
            b'+' => output.push(' '),
            b'%' => {
                let first = chars.next();
                let second = chars.next();
                if let (Some(first), Some(second)) = (first, second)
                    && let Ok(hex) = std::str::from_utf8(&[first, second])
                    && let Ok(decoded) = u8::from_str_radix(hex, 16)
                {
                    output.push(decoded as char);
                }
            }
            _ => output.push(byte as char),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_EVOLVED_REPLAY_COST, ReplayMode, ReplayRequest, ReplayRequestError,
        api_replay_response, bind_first_available, gui_smoke_check, parse_replay_request,
        replay_json, validate_replay_request,
    };

    #[test]
    fn gui_smoke_check_loads_assets_and_replay_data() {
        gui_smoke_check().expect("GUI smoke check should pass");
    }

    #[test]
    fn replay_json_contains_requested_frame_count_plus_initial_pose() {
        let json = replay_json(ReplayRequest {
            frames: 2,
            ..ReplayRequest::default()
        });
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("replay should serialize as JSON");
        let frames = value["frames"]
            .as_array()
            .expect("frames should be an array");

        assert!(value.get("body").is_some());
        assert_eq!(frames.len(), 3);
    }

    #[test]
    fn evolved_replay_default_stays_within_interactive_budget() {
        let request = ReplayRequest {
            mode: ReplayMode::Evolved,
            ..ReplayRequest::default()
        };

        assert!(validate_replay_request(&request).is_ok());
    }

    #[test]
    fn oversized_evolved_replay_returns_bad_request() {
        let request =
            parse_replay_request("mode=evolved&generations=40&population=96&evaluation_steps=500");
        let cost = 40 * 96 * 500;

        assert_eq!(
            validate_replay_request(&request),
            Err(ReplayRequestError::EvolvedBudgetExceeded {
                cost,
                max_cost: MAX_EVOLVED_REPLAY_COST
            })
        );

        let (status, content_type, body) =
            api_replay_response("mode=evolved&generations=40&population=96&evaluation_steps=500");
        let value: serde_json::Value =
            serde_json::from_str(&body).expect("error should serialize as JSON");

        assert_eq!(status, "400 Bad Request");
        assert_eq!(content_type, "application/json; charset=utf-8");
        assert_eq!(
            value["error"],
            "evolved replay request exceeds the interactive preview budget"
        );
        assert_eq!(value["cost"].as_u64(), Some(cost as u64));
        assert_eq!(
            value["max_cost"].as_u64(),
            Some(MAX_EVOLVED_REPLAY_COST as u64)
        );
    }

    #[test]
    fn bind_first_available_does_not_overflow_high_ports() {
        let result = bind_first_available("invalid host", u16::MAX);

        assert!(result.is_err());
    }

    #[test]
    fn port_probe_handles_upper_port_bound() {
        let result = bind_first_available("not-a-real-host.invalid", u16::MAX);

        assert!(result.is_err());
    }
}
