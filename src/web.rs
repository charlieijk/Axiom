use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use serde::{Deserialize, Serialize};

use crate::{
    Axis, EvolutionCheckpoint, EvolutionConfig, EvolutionReport, Genome, SearchMode, TaskKind,
    animation::capture_replay, evaluate, load_checkpoint, policy::ControllerKind, qd::Elite,
    rng::Rng, run_evolution, run_evolution_with_progress, save_checkpoint,
};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");
const GRAPHICS_HTML: &str = include_str!("../web/graphics3d.html");
const GRAPHICS_CSS: &str = include_str!("../web/graphics3d.css");
const GRAPHICS_JS: &str = include_str!("../web/graphics3d.js");
const DEFAULT_CHECKPOINT_DIR: &str = "checkpoints";
// `/api/replay?mode=evolved` is an interactive preview, not a full experiment runner.
// Defaults stay around 60k simulated evaluation steps; longer searches belong in the CLI.
const MAX_EVOLVED_REPLAY_COST: usize = 250_000;

type SharedRunStore = Arc<Mutex<RunStore>>;

#[derive(Clone, Debug)]
pub struct GuiConfig {
    pub host: String,
    pub port: u16,
    pub check: bool,
    pub checkpoint_dir: String,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8787,
            check: false,
            checkpoint_dir: DEFAULT_CHECKPOINT_DIR.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
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
    run_id: Option<String>,
    cell: Option<(usize, usize)>,
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
    EvolvedBudgetExceeded {
        cost: usize,
        max_cost: usize,
    },
    MissingRunStore,
    CellRequired,
    RunNotFound(String),
    RunNotCompleted(String),
    CellNotFound {
        run_id: String,
        cell: (usize, usize),
    },
}

#[derive(Serialize)]
struct ReplayErrorResponse {
    error: String,
    cost: Option<usize>,
    max_cost: Option<usize>,
}

#[derive(Serialize)]
struct ReplayResponse {
    controller: String,
    task: String,
    seed: u64,
    source: String,
    run_id: Option<String>,
    cell: Option<[usize; 2]>,
    genome_id: Option<u64>,
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

#[derive(Clone, Debug)]
struct RunStore {
    next_run_id: u64,
    checkpoint_dir: PathBuf,
    runs: HashMap<String, StoredRun>,
}

#[derive(Clone, Debug)]
struct StoredRun {
    id: String,
    status: RunStatus,
    config: EvolutionConfig,
    progress_generation: usize,
    evaluated_count: usize,
    best_fitness: Option<f32>,
    best_distance: Option<f32>,
    coverage: f32,
    occupied_cells: usize,
    error: Option<String>,
    checkpoint_file: Option<String>,
    report: Option<EvolutionReport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RunStatus {
    Running,
    Completed,
    Failed,
}

impl RunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Deserialize)]
struct RunStartRequest {
    seed: Option<u64>,
    population: Option<usize>,
    population_size: Option<usize>,
    generations: Option<usize>,
    evaluation_steps: Option<usize>,
    task: Option<String>,
    search_mode: Option<String>,
    archive_width: Option<usize>,
    archive_height: Option<usize>,
    x_axis: Option<String>,
    y_axis: Option<String>,
    archive_x_axis: Option<String>,
    archive_y_axis: Option<String>,
}

#[derive(Serialize)]
struct RunStartResponse {
    id: String,
    status: &'static str,
}

#[derive(Serialize)]
struct RunSummaryResponse {
    id: String,
    status: &'static str,
    progress: f32,
    generation: usize,
    total_generations: usize,
    evaluated_count: usize,
    best_fitness: Option<f32>,
    best_distance: Option<f32>,
    coverage: f32,
    occupied_cells: usize,
    error: Option<String>,
    checkpoint_file: Option<String>,
}

#[derive(Serialize)]
struct ArchiveResponse {
    run_id: String,
    width: usize,
    height: usize,
    x_axis: &'static str,
    y_axis: &'static str,
    cells: Vec<ArchiveCellResponse>,
}

#[derive(Serialize)]
struct ArchiveCellResponse {
    x: usize,
    y: usize,
    occupied: bool,
    genome_id: Option<u64>,
    parent_id: Option<u64>,
    generation: Option<usize>,
    fitness: Option<f32>,
    distance: Option<f32>,
    stable_distance: Option<f32>,
    stability: Option<f32>,
    body_count: Option<f32>,
    actuator_count: Option<f32>,
    mutation_summary: Option<String>,
    lineage: Vec<LineageStepResponse>,
}

#[derive(Serialize)]
struct LineageStepResponse {
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    controller: &'static str,
    body_count: usize,
    actuator_count: usize,
    mutation_summary: String,
}

#[derive(Serialize)]
struct CheckpointListResponse {
    directory: String,
    files: Vec<CheckpointFileResponse>,
}

#[derive(Serialize)]
struct CheckpointFileResponse {
    name: String,
    bytes: u64,
}

#[derive(Serialize)]
struct ApiErrorResponse {
    error: String,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    query: String,
    body: String,
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
            run_id: None,
            cell: None,
        }
    }
}

impl RunStore {
    fn new(checkpoint_dir: PathBuf) -> Self {
        Self {
            next_run_id: 1,
            checkpoint_dir,
            runs: HashMap::new(),
        }
    }

    fn reserve_run(&mut self, config: EvolutionConfig) -> String {
        let id = format!("run-{}", self.next_run_id);
        self.next_run_id += 1;
        self.runs
            .insert(id.clone(), StoredRun::running(&id, config));
        id
    }

    fn checkpoint_dir_label(&self) -> String {
        self.checkpoint_dir.to_string_lossy().into_owned()
    }

    fn run_summary(&self, id: &str) -> Option<RunSummaryResponse> {
        self.runs.get(id).map(StoredRun::summary)
    }

    fn archive_response(&self, id: &str) -> Result<ArchiveResponse, String> {
        let run = self
            .runs
            .get(id)
            .ok_or_else(|| format!("run not found: {id}"))?;
        let report = run
            .report
            .as_ref()
            .ok_or_else(|| format!("run is not completed: {id}"))?;
        Ok(archive_response_from_report(id, report))
    }

    fn insert_loaded_checkpoint(
        &mut self,
        file_name: String,
        checkpoint: EvolutionCheckpoint,
    ) -> String {
        let id = format!("checkpoint-{}", self.next_run_id);
        self.next_run_id += 1;
        let mut run = StoredRun::completed(&id, checkpoint.report);
        run.checkpoint_file = Some(file_name);
        self.runs.insert(id.clone(), run);
        id
    }
}

impl StoredRun {
    fn running(id: &str, config: EvolutionConfig) -> Self {
        Self {
            id: id.to_string(),
            status: RunStatus::Running,
            config,
            progress_generation: 0,
            evaluated_count: 0,
            best_fitness: None,
            best_distance: None,
            coverage: 0.0,
            occupied_cells: 0,
            error: None,
            checkpoint_file: None,
            report: None,
        }
    }

    fn completed(id: &str, report: EvolutionReport) -> Self {
        let summary = report.generation_summaries.last().cloned();
        Self {
            id: id.to_string(),
            status: RunStatus::Completed,
            config: report.config.clone(),
            progress_generation: report.generations,
            evaluated_count: report.evaluated_count,
            best_fitness: Some(report.best_evaluation.fitness),
            best_distance: Some(report.best_evaluation.metrics.distance),
            coverage: report.archive.coverage(),
            occupied_cells: report.archive.occupied_count(),
            error: None,
            checkpoint_file: None,
            report: Some(report),
        }
        .with_summary(summary.as_ref())
    }

    fn with_summary(mut self, summary: Option<&crate::GenerationSummary>) -> Self {
        if let Some(summary) = summary {
            self.progress_generation = summary.generation + 1;
            self.evaluated_count = summary.evaluated_count;
            self.best_fitness = Some(summary.best_fitness);
            self.best_distance = Some(summary.best_distance);
            self.coverage = summary.coverage;
            self.occupied_cells = summary.occupied_cells;
        }
        self
    }

    fn summary(&self) -> RunSummaryResponse {
        let total_generations = self.config.generations.max(1);
        let progress = match self.status {
            RunStatus::Completed => 1.0,
            RunStatus::Failed => 1.0,
            RunStatus::Running => {
                (self.progress_generation as f32 / total_generations as f32).clamp(0.0, 1.0)
            }
        };

        RunSummaryResponse {
            id: self.id.clone(),
            status: self.status.as_str(),
            progress,
            generation: self.progress_generation,
            total_generations,
            evaluated_count: self.evaluated_count,
            best_fitness: self.best_fitness,
            best_distance: self.best_distance,
            coverage: self.coverage,
            occupied_cells: self.occupied_cells,
            error: self.error.clone(),
            checkpoint_file: self.checkpoint_file.clone(),
        }
    }
}

pub fn serve_gui(config: GuiConfig) -> io::Result<SocketAddr> {
    let listener = bind_first_available(&config.host, config.port)?;
    let address = listener.local_addr()?;
    let store = Arc::new(Mutex::new(RunStore::new(PathBuf::from(
        config.checkpoint_dir,
    ))));
    println!("Axiom GUI running at http://{address}");
    println!("Press Ctrl-C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream.peer_addr().ok();
                let store = Arc::clone(&store);
                let worker = thread::Builder::new()
                    .name("axiom-gui-request".to_string())
                    .spawn(move || {
                        if let Err(error) = handle_connection(stream, store) {
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
    let replay = replay_json(default_replay_source(ReplayRequest {
        frames: 3,
        ..ReplayRequest::default()
    }));
    let report = run_evolution(EvolutionConfig {
        population_size: 4,
        generations: 1,
        evaluation_steps: 6,
        archive_width: 4,
        archive_height: 3,
        ..EvolutionConfig::default()
    })
    .map_err(io::Error::other)?;
    let archive = archive_response_from_report("smoke", &report);

    if !html.contains("archive-grid")
        || !html.contains("run-button")
        || !css.contains(".lab-shell")
        || !js.contains("startRun")
    {
        return Err(io::Error::other(
            "Lab assets did not include expected UI markers",
        ));
    }
    if !graphics.contains("graphics-canvas")
        || !graphics_js.contains("THREE_MODULE_URL")
        || !graphics_js.contains("run")
    {
        return Err(io::Error::other(
            "3D GUI assets did not include expected scene markers",
        ));
    }
    if !replay.contains("\"frames\"") || !replay.contains("\"bodies\"") {
        return Err(io::Error::other(
            "replay JSON did not include frame/body data",
        ));
    }
    if archive.cells.len() != report.archive.width * report.archive.height {
        return Err(io::Error::other("archive JSON did not include grid cells"));
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

fn handle_connection(mut stream: TcpStream, store: SharedRunStore) -> io::Result<()> {
    let request = read_http_request(&mut stream)?;
    let (status, content_type, body) = route_request(request, store);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 4096];
    let mut header_end = None;
    loop {
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(position) = find_header_end(&buffer) {
            header_end = Some(position);
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err(io::Error::other("request headers are too large"));
        }
    }
    let header_end = header_end.ok_or_else(|| io::Error::other("malformed HTTP request"))?;

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }

    let request_line = headers.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let body_end = (body_start + content_length).min(buffer.len());
    let body = String::from_utf8_lossy(&buffer[body_start..body_end]).to_string();

    Ok(HttpRequest {
        method,
        path: path.to_string(),
        query: query.to_string(),
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn route_request(
    request: HttpRequest,
    store: SharedRunStore,
) -> (&'static str, &'static str, String) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            ("200 OK", "text/html; charset=utf-8", response_body("/"))
        }
        ("GET", "/3d") | ("GET", "/3d.html") => {
            ("200 OK", "text/html; charset=utf-8", response_body("/3d"))
        }
        ("GET", "/styles.css") | ("GET", "/graphics3d.css") => (
            "200 OK",
            "text/css; charset=utf-8",
            response_body(&request.path),
        ),
        ("GET", "/app.js") | ("GET", "/graphics3d.js") => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(&request.path),
        ),
        ("POST", "/api/runs") => start_run_response(store, &request.body),
        ("GET", "/api/checkpoints") => checkpoint_list_response(store),
        ("GET", "/api/replay") => {
            let locked = store.lock().expect("run store should not be poisoned");
            api_replay_response(&request.query, Some(&locked))
        }
        _ => route_dynamic_api(request, store),
    }
}

fn route_dynamic_api(
    request: HttpRequest,
    store: SharedRunStore,
) -> (&'static str, &'static str, String) {
    if request.method == "GET"
        && request.path.starts_with("/api/runs/")
        && request.path.ends_with("/archive")
    {
        let id = request
            .path
            .trim_start_matches("/api/runs/")
            .trim_end_matches("/archive")
            .trim_end_matches('/');
        let locked = store.lock().expect("run store should not be poisoned");
        return match locked.archive_response(id) {
            Ok(response) => json_response("200 OK", &response),
            Err(error) => json_response("404 Not Found", &ApiErrorResponse { error }),
        };
    }

    if request.method == "GET" && request.path.starts_with("/api/runs/") {
        let id = request.path.trim_start_matches("/api/runs/");
        let locked = store.lock().expect("run store should not be poisoned");
        return match locked.run_summary(id) {
            Some(response) => json_response("200 OK", &response),
            None => json_response(
                "404 Not Found",
                &ApiErrorResponse {
                    error: format!("run not found: {id}"),
                },
            ),
        };
    }

    if request.method == "POST" && request.path.starts_with("/api/checkpoints/") {
        let name = url_decode(
            request
                .path
                .trim_start_matches("/api/checkpoints/")
                .trim_start_matches('/'),
        );
        return checkpoint_load_response(store, &name);
    }

    json_response(
        "404 Not Found",
        &ApiErrorResponse {
            error: "not found".to_string(),
        },
    )
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

fn start_run_response(store: SharedRunStore, body: &str) -> (&'static str, &'static str, String) {
    let config = match parse_run_start_config(body) {
        Ok(config) => config,
        Err(error) => return json_response("400 Bad Request", &ApiErrorResponse { error }),
    };

    let id = {
        let mut locked = store.lock().expect("run store should not be poisoned");
        locked.reserve_run(config.clone())
    };

    spawn_evolution_worker(Arc::clone(&store), id.clone(), config);
    json_response(
        "202 Accepted",
        &RunStartResponse {
            id,
            status: RunStatus::Running.as_str(),
        },
    )
}

fn parse_run_start_config(body: &str) -> Result<EvolutionConfig, String> {
    let request = if body.trim().is_empty() {
        RunStartRequest {
            seed: None,
            population: None,
            population_size: None,
            generations: None,
            evaluation_steps: None,
            task: None,
            search_mode: None,
            archive_width: None,
            archive_height: None,
            x_axis: None,
            y_axis: None,
            archive_x_axis: None,
            archive_y_axis: None,
        }
    } else {
        serde_json::from_str::<RunStartRequest>(body)
            .map_err(|error| format!("invalid run request JSON: {error}"))?
    };

    let mut config = EvolutionConfig::default();
    if let Some(seed) = request.seed {
        config.seed = seed;
    }
    if let Some(population) = request.population_size.or(request.population) {
        config.population_size = population.clamp(4, 96);
    }
    if let Some(generations) = request.generations {
        config.generations = generations.clamp(1, 40);
    }
    if let Some(evaluation_steps) = request.evaluation_steps {
        config.evaluation_steps = evaluation_steps.clamp(20, 500);
    }
    if let Some(task) = request.task {
        config.task = TaskKind::parse(&task).ok_or_else(|| format!("unknown task: {task}"))?;
    }
    if let Some(search_mode) = request.search_mode {
        config.search_mode = SearchMode::parse(&search_mode)
            .ok_or_else(|| format!("unknown search mode: {search_mode}"))?;
    }
    if let Some(width) = request.archive_width {
        config.archive_width = width.clamp(2, 32);
    }
    if let Some(height) = request.archive_height {
        config.archive_height = height.clamp(2, 24);
    }
    if let Some(axis) = request.archive_x_axis.or(request.x_axis) {
        config.archive_x_axis =
            Axis::parse(&axis).ok_or_else(|| format!("unknown x-axis: {axis}"))?;
    }
    if let Some(axis) = request.archive_y_axis.or(request.y_axis) {
        config.archive_y_axis =
            Axis::parse(&axis).ok_or_else(|| format!("unknown y-axis: {axis}"))?;
    }

    config.validate().map_err(|error| error.to_string())?;
    Ok(config)
}

fn spawn_evolution_worker(store: SharedRunStore, id: String, config: EvolutionConfig) {
    thread::spawn(move || {
        let result = run_evolution_with_progress(config, |summary| {
            let mut locked = store.lock().expect("run store should not be poisoned");
            if let Some(run) = locked.runs.get_mut(&id) {
                run.progress_generation = summary.generation + 1;
                run.evaluated_count = summary.evaluated_count;
                run.best_fitness = Some(summary.best_fitness);
                run.best_distance = Some(summary.best_distance);
                run.coverage = summary.coverage;
                run.occupied_cells = summary.occupied_cells;
            }
        });

        let mut locked = store.lock().expect("run store should not be poisoned");
        match result {
            Ok(report) => {
                let checkpoint_file = format!("{id}.json");
                let checkpoint_path = locked.checkpoint_dir.join(&checkpoint_file);
                let checkpoint_result = save_checkpoint(
                    &checkpoint_path,
                    &EvolutionCheckpoint::from_report(report.clone()),
                );
                if let Some(run) = locked.runs.get_mut(&id) {
                    *run = StoredRun::completed(&id, report);
                    run.checkpoint_file = Some(checkpoint_file);
                    if let Err(error) = checkpoint_result {
                        run.error = Some(format!("checkpoint write failed: {error}"));
                    }
                }
            }
            Err(error) => {
                if let Some(run) = locked.runs.get_mut(&id) {
                    run.status = RunStatus::Failed;
                    run.error = Some(error.to_string());
                }
            }
        }
    });
}

fn checkpoint_list_response(store: SharedRunStore) -> (&'static str, &'static str, String) {
    let locked = store.lock().expect("run store should not be poisoned");
    let files = match fs::read_dir(&locked.checkpoint_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let metadata = entry.metadata().ok()?;
                safe_checkpoint_name(&name).map(|name| CheckpointFileResponse {
                    name,
                    bytes: metadata.len(),
                })
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return json_response(
                "500 Internal Server Error",
                &ApiErrorResponse {
                    error: format!("checkpoint list failed: {error}"),
                },
            );
        }
    };

    let mut files = files;
    files.sort_by(|a, b| a.name.cmp(&b.name));
    json_response(
        "200 OK",
        &CheckpointListResponse {
            directory: locked.checkpoint_dir_label(),
            files,
        },
    )
}

fn checkpoint_load_response(
    store: SharedRunStore,
    name: &str,
) -> (&'static str, &'static str, String) {
    let Some(name) = safe_checkpoint_name(name) else {
        return json_response(
            "400 Bad Request",
            &ApiErrorResponse {
                error: "invalid checkpoint file name".to_string(),
            },
        );
    };

    let checkpoint_path = {
        let locked = store.lock().expect("run store should not be poisoned");
        locked.checkpoint_dir.join(&name)
    };

    let checkpoint = match load_checkpoint(&checkpoint_path) {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            return json_response(
                "404 Not Found",
                &ApiErrorResponse {
                    error: format!("checkpoint load failed: {error}"),
                },
            );
        }
    };

    let response = {
        let mut locked = store.lock().expect("run store should not be poisoned");
        let id = locked.insert_loaded_checkpoint(name, checkpoint);
        locked
            .run_summary(&id)
            .expect("loaded checkpoint run was just inserted")
    };
    json_response("200 OK", &response)
}

fn safe_checkpoint_name(name: &str) -> Option<String> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || !name.ends_with(".json")
    {
        return None;
    }
    let file_name = std::path::Path::new(name).file_name()?.to_str()?;
    (file_name == name).then(|| name.to_string())
}

fn archive_response_from_report(run_id: &str, report: &EvolutionReport) -> ArchiveResponse {
    let mut cells = Vec::with_capacity(report.archive.width * report.archive.height);
    for y in 0..report.archive.height {
        for x in 0..report.archive.width {
            cells.push(match report.archive.elite_at((x, y)) {
                Some(elite) => archive_cell_response(x, y, Some(elite), report),
                None => archive_cell_response(x, y, None, report),
            });
        }
    }

    ArchiveResponse {
        run_id: run_id.to_string(),
        width: report.archive.width,
        height: report.archive.height,
        x_axis: report.archive.x_axis.as_str(),
        y_axis: report.archive.y_axis.as_str(),
        cells,
    }
}

fn archive_cell_response(
    x: usize,
    y: usize,
    elite: Option<&Elite>,
    report: &EvolutionReport,
) -> ArchiveCellResponse {
    let metrics = elite.map(|elite| &elite.evaluation.metrics);
    ArchiveCellResponse {
        x,
        y,
        occupied: elite.is_some(),
        genome_id: elite.map(|elite| elite.genome_id),
        parent_id: elite.and_then(|elite| elite.parent_id),
        generation: elite.map(|elite| elite.generation),
        fitness: elite.map(|elite| elite.evaluation.fitness),
        distance: metrics.map(|metrics| metrics.distance),
        stable_distance: metrics.map(|metrics| metrics.stable_distance),
        stability: metrics.map(|metrics| metrics.stability),
        body_count: metrics.map(|metrics| metrics.body_count),
        actuator_count: metrics.map(|metrics| metrics.actuator_count),
        mutation_summary: elite.map(|elite| elite.mutation_summary.clone()),
        lineage: elite
            .map(|elite| lineage_steps(elite.genome_id, report))
            .unwrap_or_default(),
    }
}

fn lineage_steps(genome_id: u64, report: &EvolutionReport) -> Vec<LineageStepResponse> {
    let records: HashMap<_, _> = report
        .lineage
        .iter()
        .map(|record| (record.genome_id, record))
        .collect();
    let mut steps = Vec::new();
    let mut current = Some(genome_id);
    while let Some(id) = current {
        let Some(record) = records.get(&id) else {
            break;
        };
        steps.push(LineageStepResponse {
            genome_id: record.genome_id,
            parent_id: record.parent_id,
            generation: record.generation,
            controller: record.controller.as_str(),
            body_count: record.body_count,
            actuator_count: record.actuator_count,
            mutation_summary: record.mutation_summary.clone(),
        });
        current = record.parent_id;
        if steps.len() >= 64 {
            break;
        }
    }
    steps.reverse();
    steps
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
                if let Some(controller) = ControllerKind::parse(&value) {
                    request.controller = controller;
                }
            }
            "task" => {
                if let Some(task) = TaskKind::parse(&value) {
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
            "run" | "run_id" => {
                if !value.is_empty() {
                    request.run_id = Some(value);
                }
            }
            "cell" => {
                request.cell = parse_cell(&value);
            }
            _ => {}
        }
    }

    request
}

fn api_replay_response(
    query: &str,
    store: Option<&RunStore>,
) -> (&'static str, &'static str, String) {
    let request = parse_replay_request(query);
    let source = if request.run_id.is_some() {
        archive_replay_source(&request, store)
    } else {
        validate_replay_request(&request).map(|()| default_replay_source(request))
    };

    match source {
        Ok(source) => (
            "200 OK",
            "application/json; charset=utf-8",
            replay_json(source),
        ),
        Err(error) => {
            let status = match error {
                ReplayRequestError::EvolvedBudgetExceeded { .. }
                | ReplayRequestError::CellRequired
                | ReplayRequestError::MissingRunStore => "400 Bad Request",
                ReplayRequestError::RunNotFound(_) | ReplayRequestError::CellNotFound { .. } => {
                    "404 Not Found"
                }
                ReplayRequestError::RunNotCompleted(_) => "409 Conflict",
            };
            (
                status,
                "application/json; charset=utf-8",
                replay_error_json(error),
            )
        }
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
    let (error, cost, max_cost) = match error {
        ReplayRequestError::EvolvedBudgetExceeded { cost, max_cost } => (
            "evolved replay request exceeds the interactive preview budget".to_string(),
            Some(cost),
            Some(max_cost),
        ),
        ReplayRequestError::MissingRunStore => (
            "archive replay requires an active run store".to_string(),
            None,
            None,
        ),
        ReplayRequestError::CellRequired => (
            "archive replay requires a cell=x,y query".to_string(),
            None,
            None,
        ),
        ReplayRequestError::RunNotFound(id) => (format!("run not found: {id}"), None, None),
        ReplayRequestError::RunNotCompleted(id) => {
            (format!("run is not completed: {id}"), None, None)
        }
        ReplayRequestError::CellNotFound { run_id, cell } => (
            format!("cell {},{} is empty for run {run_id}", cell.0, cell.1),
            None,
            None,
        ),
    };
    serde_json::to_string(&ReplayErrorResponse {
        error,
        cost,
        max_cost,
    })
    .expect("serializing replay error cannot fail")
}

#[derive(Clone, Debug)]
struct ReplaySource {
    genome: Genome,
    evaluation: crate::Evaluation,
    task: TaskKind,
    seed: u64,
    source: String,
    run_id: Option<String>,
    cell: Option<(usize, usize)>,
    genome_id: Option<u64>,
    generations: usize,
    population_size: usize,
    evaluation_steps: usize,
    frames: usize,
    dt: f32,
}

fn default_replay_source(request: ReplayRequest) -> ReplaySource {
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
            })
            .expect("validated replay evolution config should be valid");
            (
                report.best_genome,
                report.best_evaluation,
                request.generations,
                request.population_size,
            )
        }
    };

    ReplaySource {
        genome,
        evaluation,
        task: request.task,
        seed: request.seed,
        source: request.mode.as_str().to_string(),
        run_id: None,
        cell: None,
        genome_id: None,
        generations,
        population_size,
        evaluation_steps: request.evaluation_steps,
        frames: request.frames,
        dt: request.dt,
    }
}

fn archive_replay_source(
    request: &ReplayRequest,
    store: Option<&RunStore>,
) -> Result<ReplaySource, ReplayRequestError> {
    let store = store.ok_or(ReplayRequestError::MissingRunStore)?;
    let run_id = request
        .run_id
        .as_ref()
        .expect("checked by caller")
        .to_string();
    let cell = request.cell.ok_or(ReplayRequestError::CellRequired)?;
    let run = store
        .runs
        .get(&run_id)
        .ok_or_else(|| ReplayRequestError::RunNotFound(run_id.clone()))?;
    let report = run
        .report
        .as_ref()
        .ok_or_else(|| ReplayRequestError::RunNotCompleted(run_id.clone()))?;
    let elite = report
        .archive
        .elite_at(cell)
        .ok_or_else(|| ReplayRequestError::CellNotFound {
            run_id: run_id.clone(),
            cell,
        })?;

    Ok(ReplaySource {
        genome: elite.genome.clone(),
        evaluation: elite.evaluation.clone(),
        task: report.config.task,
        seed: report.config.seed,
        source: "archive".to_string(),
        run_id: Some(run_id),
        cell: Some(elite.cell),
        genome_id: Some(elite.genome_id),
        generations: report.generations,
        population_size: report.config.population_size,
        evaluation_steps: report.config.evaluation_steps,
        frames: request.frames,
        dt: request.dt,
    })
}

fn replay_json(source: ReplaySource) -> String {
    let frames = capture_replay(&source.genome, source.task, source.frames, source.dt);
    let response = ReplayResponse {
        controller: source.genome.controller.as_str().to_string(),
        task: source.task.as_str().to_string(),
        seed: source.seed,
        source: source.source,
        run_id: source.run_id,
        cell: source.cell.map(|cell| [cell.0, cell.1]),
        genome_id: source.genome_id,
        generations: source.generations,
        population: source.population_size,
        evaluation_steps: source.evaluation_steps,
        fitness: source.evaluation.fitness,
        best_distance: source.evaluation.metrics.distance,
        stable_distance: source.evaluation.metrics.stable_distance,
        uprightness: source.evaluation.metrics.uprightness,
        stability: source.evaluation.metrics.stability,
        terminal_tilt: source.evaluation.metrics.terminal_tilt,
        dt: source.dt,
        body: source
            .genome
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

fn parse_cell(value: &str) -> Option<(usize, usize)> {
    let (x, y) = value.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn json_response<T: Serialize>(
    status: &'static str,
    value: &T,
) -> (&'static str, &'static str, String) {
    (
        status,
        "application/json; charset=utf-8",
        serde_json::to_string(value).expect("serializing API response cannot fail"),
    )
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
        MAX_EVOLVED_REPLAY_COST, ReplayMode, ReplayRequest, ReplayRequestError, RunStore,
        api_replay_response, archive_response_from_report, bind_first_available, gui_smoke_check,
        parse_replay_request, replay_json, safe_checkpoint_name, validate_replay_request,
    };
    use crate::{EvolutionConfig, evolution::run_evolution};
    use std::path::PathBuf;

    #[test]
    fn gui_smoke_check_loads_assets_and_replay_data() {
        gui_smoke_check().expect("GUI smoke check should pass");
    }

    #[test]
    fn replay_json_contains_requested_frame_count_plus_initial_pose() {
        let source = super::default_replay_source(ReplayRequest {
            frames: 2,
            ..ReplayRequest::default()
        });
        let json = replay_json(source);
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

        let (status, content_type, body) = api_replay_response(
            "mode=evolved&generations=40&population=96&evaluation_steps=500",
            None,
        );
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
    fn archive_replay_returns_selected_cell_json() {
        let report = run_evolution(EvolutionConfig {
            population_size: 6,
            generations: 1,
            evaluation_steps: 8,
            archive_width: 4,
            archive_height: 4,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");
        let elite = report
            .archive
            .elites()
            .next()
            .expect("archive should have elite");
        let query = format!("run=run-1&cell={},{}&frames=2", elite.cell.0, elite.cell.1);
        let mut store = RunStore::new(PathBuf::from("checkpoints"));
        store.runs.insert(
            "run-1".to_string(),
            super::StoredRun::completed("run-1", report),
        );

        let (status, _, body) = api_replay_response(&query, Some(&store));
        let value: serde_json::Value =
            serde_json::from_str(&body).expect("replay should serialize as JSON");

        assert_eq!(status, "200 OK");
        assert_eq!(value["source"], "archive");
        assert_eq!(value["frames"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn invalid_archive_replay_run_returns_structured_error() {
        let store = RunStore::new(PathBuf::from("checkpoints"));

        let (status, _, body) = api_replay_response("run=missing&cell=0,0", Some(&store));
        let value: serde_json::Value =
            serde_json::from_str(&body).expect("error should serialize as JSON");

        assert_eq!(status, "404 Not Found");
        assert_eq!(value["error"], "run not found: missing");
    }

    #[test]
    fn archive_response_includes_every_grid_cell() {
        let report = run_evolution(EvolutionConfig {
            population_size: 4,
            generations: 1,
            evaluation_steps: 8,
            archive_width: 5,
            archive_height: 3,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");

        let response = archive_response_from_report("run-1", &report);

        assert_eq!(response.cells.len(), 15);
        assert!(response.cells.iter().any(|cell| cell.occupied));
        assert!(
            response
                .cells
                .iter()
                .filter(|cell| cell.occupied)
                .all(|cell| !cell.lineage.is_empty())
        );
    }

    #[test]
    fn checkpoint_filename_sanitizer_rejects_path_traversal() {
        assert_eq!(
            safe_checkpoint_name("run.json"),
            Some("run.json".to_string())
        );
        assert_eq!(safe_checkpoint_name("../run.json"), None);
        assert_eq!(safe_checkpoint_name("nested/run.json"), None);
        assert_eq!(safe_checkpoint_name("run.txt"), None);
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
