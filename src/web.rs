use std::{
    fmt::Write as FmtWrite,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

use crate::{Genome, TaskKind, animation::capture_replay, policy::ControllerKind, rng::Rng};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");

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
    controller: ControllerKind,
    task: TaskKind,
    seed: u64,
    frames: usize,
    dt: f32,
}

impl Default for ReplayRequest {
    fn default() -> Self {
        Self {
            controller: ControllerKind::Cpg,
            task: TaskKind::RoughTerrain,
            seed: 19,
            frames: 220,
            dt: 0.05,
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
            Ok(stream) => handle_connection(stream)?,
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(address)
}

pub fn gui_smoke_check() -> io::Result<()> {
    let html = response_body("/");
    let css = response_body("/styles.css");
    let js = response_body("/app.js");
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
        let address = format!("{host}:{}", preferred_port + offset);
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
        "/styles.css" => ("200 OK", "text/css; charset=utf-8", response_body(path)),
        "/app.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
        ),
        "/api/replay" => (
            "200 OK",
            "application/json; charset=utf-8",
            replay_json(parse_replay_request(query)),
        ),
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
        "/styles.css" => APP_CSS.to_string(),
        "/app.js" => APP_JS.to_string(),
        _ => String::new(),
    }
}

fn parse_replay_request(query: &str) -> ReplayRequest {
    let mut request = ReplayRequest::default();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = url_decode(value);
        match key {
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
            _ => {}
        }
    }

    request
}

fn replay_json(request: ReplayRequest) -> String {
    let mut rng = Rng::new(request.seed);
    let genome = Genome::minimal(request.controller, &mut rng);
    let frames = capture_replay(&genome, request.task, request.frames, request.dt);
    let mut json = String::new();

    write!(
        json,
        "{{\"controller\":\"{}\",\"task\":\"{}\",\"seed\":{},\"dt\":{},\"body\":[",
        request.controller.as_str(),
        task_name(request.task),
        request.seed,
        request.dt
    )
    .expect("writing to a String cannot fail");

    for (index, node) in genome.body.nodes.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"id\":{},\"parent\":{},\"size\":[{:.4},{:.4}],\"actuator\":{:.4}}}",
            node.id,
            node.parent
                .map(|parent| parent.to_string())
                .unwrap_or_else(|| "null".to_string()),
            node.size.x,
            node.size.y,
            node.actuator_strength
        )
        .expect("writing to a String cannot fail");
    }

    json.push_str("],\"frames\":[");
    for (frame_index, frame) in frames.iter().enumerate() {
        if frame_index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"time\":{:.4},\"root\":[{:.4},{:.4}],\"tilt\":{:.4},\"bodies\":[",
            frame.time, frame.root_position.x, frame.root_position.y, frame.tilt
        )
        .expect("writing to a String cannot fail");

        for (body_index, center) in frame.body_centers.iter().enumerate() {
            if body_index > 0 {
                json.push(',');
            }
            write!(json, "[{:.4},{:.4}]", center.x, center.y)
                .expect("writing to a String cannot fail");
        }

        json.push_str("],\"joints\":[");
        for (joint_index, (start, end)) in frame.joint_segments.iter().enumerate() {
            if joint_index > 0 {
                json.push(',');
            }
            write!(
                json,
                "[[{:.4},{:.4}],[{:.4},{:.4}]]",
                start.x, start.y, end.x, end.y
            )
            .expect("writing to a String cannot fail");
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    json
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
    use super::{ReplayRequest, gui_smoke_check, replay_json};

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

        assert!(json.contains("\"body\""));
        assert_eq!(json.matches("\"time\"").count(), 3);
    }
}
