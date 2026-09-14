//! Single-user, loopback-only test range over the real rigid-body simulator.
//! The browser sends bounded gait inputs. Only Rust advances physics or scores a run.
use crate::{
    FieldSim, Gait, Sample,
    sim::BodyPose,
    workshop::{RobotDesign, Save, SearchInput, Workshop},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

const ADDRESS: &str = "127.0.0.1:8790";
const ORIGIN: &str = "http://127.0.0.1:8790";
const GOAL_M: f32 = 1.0;
const MAX_TICKS: usize = 1200;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Course {
    #[default]
    Flat,
    Rails,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GaitInput {
    pub frequency_hz: f32,
    pub hip_amplitude: f32,
    pub knee_amplitude: f32,
}
impl Default for GaitInput {
    fn default() -> Self {
        let gait = Gait::default();
        Self {
            frequency_hz: gait.frequency_hz,
            hip_amplitude: gait.hip_amplitude,
            knee_amplitude: gait.knee_amplitude,
        }
    }
}
impl GaitInput {
    pub(crate) fn valid(self) -> bool {
        (0.25..=4.0).contains(&self.frequency_hz)
            && (0.0..=0.45).contains(&self.hip_amplitude)
            && (0.0..=0.45).contains(&self.knee_amplitude)
    }
    pub(crate) fn gait(self) -> Gait {
        Gait {
            frequency_hz: self.frequency_hz,
            hip_amplitude: self.hip_amplitude,
            knee_amplitude: self.knee_amplitude,
            ..Gait::default()
        }
    }
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub sample: Sample,
    pub bodies: Vec<BodyPose>,
    pub elapsed_s: f32,
    pub distance_m: f32,
    pub outcome: &'static str,
    pub course: Course,
    pub gait: GaitInput,
    pub goal_m: f32,
    pub step_seconds: f32,
    pub design: RobotDesign,
    pub controller_name: String,
    pub controller_mode: &'static str,
    pub controller_frequency_hz: f32,
}
#[derive(Clone, Serialize)]
struct Recording {
    id: u64,
    course: Course,
    controller_name: String,
    design: RobotDesign,
    frames: Vec<Snapshot>,
}
pub struct Lab {
    sim: FieldSim,
    start_x: f32,
    ticks: usize,
    phase_cycles: f32,
    gait: GaitInput,
    course: Course,
    outcome: &'static str,
    workshop: Workshop,
    frames: Vec<Snapshot>,
    recordings: Vec<Recording>,
    run_id: u64,
}
impl Lab {
    pub fn new(course: Course) -> Self {
        let workshop = Workshop::default();
        let mut sim = FieldSim::new(workshop.design.robot().expect("default design"));
        if course == Course::Rails {
            sim.add_test_rails();
        }
        let start_x = sim.settle(100).forward_m();
        let mut lab = Self {
            sim,
            start_x,
            ticks: 0,
            phase_cycles: 0.0,
            gait: GaitInput::default(),
            course,
            outcome: "ready",
            workshop,
            frames: Vec::new(),
            recordings: Vec::new(),
            run_id: 1,
        };
        lab.frames.push(lab.snapshot());
        lab
    }
    fn reset_run(&mut self, course: Course) {
        self.save_recording();
        self.sim = FieldSim::new(
            self.workshop
                .design
                .robot()
                .expect("validated workshop design"),
        );
        if course == Course::Rails {
            self.sim.add_test_rails();
        }
        self.start_x = self.sim.settle(100).forward_m();
        self.ticks = 0;
        self.phase_cycles = 0.0;
        self.course = course;
        self.outcome = "ready";
        self.run_id += 1;
        self.frames = vec![self.snapshot()];
    }
    fn save_recording(&mut self) {
        if self.frames.len() < 2 {
            return;
        }
        // Use the frame's provenance: design/controller may already have changed
        // when a reset archives the previous physical run.
        let initial = &self.frames[0];
        let recording = Recording {
            id: self.run_id,
            course: initial.course,
            controller_name: initial.controller_name.clone(),
            design: initial.design,
            frames: self.frames.clone(),
        };
        self.recordings.retain(|r| r.id != self.run_id);
        self.recordings.push(recording);
        if self.recordings.len() > 3 {
            self.recordings.remove(0);
        }
    }
    fn recording_list(&self) -> serde_json::Value {
        serde_json::Value::Array(self.recordings.iter().rev().map(|r| {
            let end=r.frames.last().expect("recorded frame");
            serde_json::json!({"id":r.id,"course":r.course,"controller_name":r.controller_name,
                "design":r.design,"elapsed_s":end.elapsed_s,"distance_m":end.distance_m,
                "outcome":end.outcome,"frames":r.frames.len()})
        }).collect())
    }
    pub fn snapshot(&self) -> Snapshot {
        let sample = self.sim.sample();
        Snapshot {
            distance_m: sample.forward_m() - self.start_x,
            sample,
            bodies: self.sim.body_poses(),
            elapsed_s: self.ticks as f32 * self.sim.config().sim.control_dt(),
            outcome: self.outcome,
            course: self.course,
            gait: self.gait,
            goal_m: GOAL_M,
            step_seconds: self.sim.config().sim.control_dt(),
            design: self.workshop.design,
            controller_name: self.workshop.controller_name.clone(),
            controller_mode: self.workshop.mode(),
            controller_frequency_hz: self
                .workshop
                .controller
                .as_ref()
                .map_or(self.gait.frequency_hz, |g| g.frequency_hz),
        }
    }
    pub fn step(&mut self, input: GaitInput) -> Result<Snapshot, &'static str> {
        if !input.valid() {
            return Err("Gait is outside the test range limits");
        }
        if matches!(self.outcome, "complete" | "fallen" | "timeout") {
            return Ok(self.snapshot());
        }
        let (actions, frequency) = if let Some(genome) = &self.workshop.controller {
            (
                genome.actions_at(self.phase_cycles / genome.frequency_hz),
                genome.frequency_hz,
            )
        } else {
            self.gait = input;
            (
                input
                    .gait()
                    .actions_at(self.phase_cycles / input.frequency_hz),
                input.frequency_hz,
            )
        };
        let sample = self.sim.control_step(&actions);
        self.phase_cycles =
            (self.phase_cycles + frequency * self.sim.config().sim.control_dt()) % 1.0;
        self.ticks += 1;
        self.outcome = if sample.height_m() < self.sim.config().standing_height_m() * 0.6
            || sample.tilt_rad > 0.5
        {
            "fallen"
        } else if sample.forward_m() - self.start_x >= GOAL_M {
            "complete"
        } else if self.ticks >= MAX_TICKS {
            "timeout"
        } else {
            "running"
        };
        let snapshot = self.snapshot();
        self.frames.push(snapshot.clone());
        if matches!(self.outcome, "complete" | "fallen" | "timeout") {
            self.save_recording();
        }
        Ok(snapshot)
    }
}

struct Response {
    status: &'static str,
    mime: &'static str,
    body: String,
}
fn response(status: &'static str, mime: &'static str, body: impl Into<String>) -> Response {
    Response {
        status,
        mime,
        body: body.into(),
    }
}
fn json(value: &impl Serialize) -> Response {
    response(
        "200 OK",
        "application/json",
        serde_json::to_string(value).expect("finite simulation snapshot"),
    )
}
fn error(status: &'static str, message: &str) -> Response {
    response(
        status,
        "application/json",
        serde_json::json!({"error":message}).to_string(),
    )
}

fn route(
    lab: &mut Lab,
    method: &str,
    path: &str,
    host: &str,
    origin: Option<&str>,
    body: &str,
) -> Response {
    // Fail closed against DNS rebinding, cross-origin commands and stray browser forms.
    if host != ADDRESS {
        return error("403 Forbidden", "Invalid host");
    }
    if method == "POST" && origin != Some(ORIGIN) {
        return error("403 Forbidden", "Same-origin requests required");
    }
    lab.workshop.poll();
    if let Some(id) = path.strip_prefix("/api/recording/") {
        return if method == "GET" {
            match id
                .parse::<u64>()
                .ok()
                .and_then(|id| lab.recordings.iter().find(|r| r.id == id))
            {
                Some(recording) => json(recording),
                None => error("404 Not Found", "Recording not found"),
            }
        } else {
            error("405 Method Not Allowed", "Use GET for recordings")
        };
    }
    match (method, path) {
        ("GET", "/") => response(
            "200 OK",
            "text/html; charset=utf-8",
            include_str!("../web/index.html"),
        ),
        ("GET", "/lab.css") => response("200 OK", "text/css", include_str!("../web/lab.css")),
        ("GET", "/workshop.js") => response(
            "200 OK",
            "text/javascript",
            include_str!("../web/workshop.js"),
        ),
        ("GET", "/workshop-model.js") => response(
            "200 OK",
            "text/javascript",
            include_str!("../web/workshop-model.js"),
        ),
        ("GET", "/lab.js") => response("200 OK", "text/javascript", include_str!("../web/lab.js")),
        ("GET", "/lab-model.js") => response(
            "200 OK",
            "text/javascript",
            include_str!("../web/lab-model.js"),
        ),
        ("GET", "/vendor/three.module.min.js") => response(
            "200 OK",
            "text/javascript",
            include_str!("../../../web/vendor/three.module.min.js"),
        ),
        ("GET", "/api/state") => json(&lab.snapshot()),
        ("GET", "/api/workshop") => json(&lab.workshop.state()),
        ("GET", "/api/export") => json(&lab.workshop.save(lab.gait)),
        ("GET", "/api/recordings") => json(&lab.recording_list()),
        ("POST", "/api/recordings/save") => {
            if lab.ticks == 0 {
                return error("400 Bad Request", "Run the robot before saving a recording");
            }
            lab.save_recording();
            json(&lab.recording_list())
        }
        ("POST", "/api/bundled") => {
            if lab.workshop.busy() {
                return error(
                    "409 Conflict",
                    "Cancel or finish the current job before loading the bundled archive",
                );
            }
            match Workshop::bundled() {
                Ok(workshop) => {
                    lab.workshop = workshop;
                    lab.gait = GaitInput::default();
                    lab.reset_run(lab.course);
                    json(&lab.snapshot())
                }
                Err(message) => error("500 Internal Server Error", message),
            }
        }
        ("POST", "/api/design") => {
            if lab.workshop.busy() {
                return error(
                    "409 Conflict",
                    "Cancel or finish the current job before changing robot design",
                );
            }
            match serde_json::from_str::<RobotDesign>(body) {
                Ok(design) => match design.robot() {
                    Ok(_) => {
                        lab.workshop = Workshop::default();
                        lab.workshop.design = design;
                        lab.gait = GaitInput::default();
                        lab.reset_run(lab.course);
                        json(&lab.snapshot())
                    }
                    Err(message) => error("400 Bad Request", message),
                },
                Err(_) => error("400 Bad Request", "Invalid robot design JSON"),
            }
        }
        ("POST", "/api/controller") => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Selection {
                cell: Option<(usize, usize)>,
                gait: Option<GaitInput>,
            }
            match serde_json::from_str::<Selection>(body) {
                Ok(Selection {
                    cell: Some(cell),
                    gait: None,
                }) => {
                    let selected = lab
                        .workshop
                        .archive
                        .as_ref()
                        .and_then(|a| a.cells.iter().find(|e| e.cell == cell))
                        .cloned();
                    match selected {
                        Some(elite) => {
                            lab.workshop.controller = Some(elite.genome);
                            lab.workshop.controller_name =
                                format!("Archive gait {},{}", cell.0, cell.1);
                            lab.reset_run(lab.course);
                            json(&lab.snapshot())
                        }
                        None => error("400 Bad Request", "Choose an occupied archive cell"),
                    }
                }
                Ok(Selection {
                    cell: None,
                    gait: Some(gait),
                }) if gait.valid() => {
                    lab.workshop.controller = None;
                    lab.workshop.controller_name = "Manual trot".into();
                    lab.gait = gait;
                    lab.reset_run(lab.course);
                    json(&lab.snapshot())
                }
                _ => error(
                    "400 Bad Request",
                    "Choose exactly one valid gait or archive cell",
                ),
            }
        }
        ("POST", "/api/search") => match serde_json::from_str::<SearchInput>(body) {
            Ok(input) => match lab.workshop.start_search(input) {
                Ok(()) => json(&lab.workshop.state()),
                Err(message) => error("400 Bad Request", message),
            },
            Err(_) => error("400 Bad Request", "Invalid search settings"),
        },
        ("POST", "/api/cancel") => {
            lab.workshop.cancel();
            json(&lab.workshop.state())
        }
        ("POST", "/api/holdout") => match lab.workshop.start_holdout() {
            Ok(()) => json(&lab.workshop.state()),
            Err(message) => error("400 Bad Request", message),
        },
        ("POST", "/api/import") => {
            if lab.workshop.busy() {
                return error(
                    "409 Conflict",
                    "Cancel or finish the current job before importing",
                );
            }
            match serde_json::from_str::<Save>(body) {
                Ok(save) => {
                    let gait = save.gait;
                    match Workshop::restore(save) {
                        Ok(workshop) => {
                            lab.workshop = workshop;
                            lab.gait = gait;
                            lab.reset_run(lab.course);
                            json(&lab.snapshot())
                        }
                        Err(message) => error("400 Bad Request", message),
                    }
                }
                Err(_) => error("400 Bad Request", "Invalid workshop save JSON"),
            }
        }
        ("POST", "/api/step") => match serde_json::from_str::<GaitInput>(body) {
            Ok(input) => match lab.step(input) {
                Ok(state) => json(&state),
                Err(message) => error("400 Bad Request", message),
            },
            Err(_) => error("400 Bad Request", "Invalid gait JSON"),
        },
        ("POST", "/api/reset") => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Reset {
                course: Course,
            }
            match serde_json::from_str::<Reset>(body) {
                Ok(input) => {
                    lab.reset_run(input.course);
                    json(&lab.snapshot())
                }
                Err(_) => error("400 Bad Request", "Invalid course JSON"),
            }
        }
        _ => error("404 Not Found", "Route not found"),
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<(String, String)> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.len() > 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Headers too large",
            ));
        }
    }
    let header = String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid header"))?;
    let mut length = None;
    for line in header.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("transfer-encoding") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Transfer encoding unsupported",
                ));
            }
            if name.eq_ignore_ascii_case("content-length") {
                if length.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Duplicate length",
                    ));
                }
                length =
                    Some(value.trim().parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid length")
                    })?);
            }
        }
    }
    let count = length.unwrap_or(0);
    if count > 256 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Body too large"));
    }
    let mut body = vec![0u8; count];
    stream.read_exact(&mut body)?;
    Ok((
        header,
        String::from_utf8(body)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid body"))?,
    ))
}
fn dispatch(lab: &mut Lab, header: &str, body: &str) -> Response {
    let mut lines = header.lines();
    let first = lines.next().unwrap_or_default();
    let parts: Vec<_> = first.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return error("400 Bad Request", "Invalid request");
    }
    let mut host = None;
    let mut origin = None;
    let mut content_type = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return error("400 Bad Request", "Invalid header");
        };
        let slot = if key.eq_ignore_ascii_case("host") {
            Some(&mut host)
        } else if key.eq_ignore_ascii_case("origin") {
            Some(&mut origin)
        } else if key.eq_ignore_ascii_case("content-type") {
            Some(&mut content_type)
        } else {
            None
        };
        if let Some(slot) = slot {
            if slot.is_some() {
                return error("400 Bad Request", "Duplicate header");
            }
            *slot = Some(value.trim());
        }
    }
    if parts[0] == "POST" && content_type != Some("application/json") {
        return error("415 Unsupported Media Type", "Use application/json");
    }
    route(
        lab,
        parts[0],
        parts[1],
        host.unwrap_or_default(),
        origin,
        body,
    )
}
fn write_response(stream: &mut TcpStream, result: Response) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'\r\n\r\n{}",
        result.status,
        result.mime,
        result.body.len(),
        result.body
    )
}
pub fn serve() -> io::Result<()> {
    let listener = TcpListener::bind(ADDRESS)?;
    let mut lab = Lab::new(Course::Flat);
    println!("Axiom: {ORIGIN} — local, software-only physics. Ctrl-C to stop.");
    for stream in listener.incoming() {
        let mut stream = stream?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let result = match read_request(&mut stream) {
            Ok((headers, body)) => dispatch(&mut lab, &headers, &body),
            Err(_) => error("400 Bad Request", "Invalid or oversized request"),
        };
        if let Err(error) = write_response(&mut stream, result) {
            eprintln!("test range connection: {error}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_comes_from_the_physics_world() {
        let mut lab = Lab::new(Course::Flat);
        let before = lab.snapshot();
        assert_eq!(before.bodies.len(), 10);
        assert_eq!(before.bodies.iter().filter(|p| p.kind == "limb").count(), 8);
        let chassis = before.bodies.iter().find(|p| p.kind == "chassis").unwrap();
        assert_eq!(chassis.position, before.sample.position_m);
        for _ in 0..30 {
            lab.step(GaitInput::default()).unwrap();
        }
        let after = lab.snapshot();
        assert_ne!(before.bodies[2].rotation, after.bodies[2].rotation);
        assert_eq!(after.elapsed_s, 1.5);
    }
    #[test]
    fn default_gait_can_complete_the_measured_challenge() {
        let mut lab = Lab::new(Course::Flat);
        for _ in 0..MAX_TICKS {
            lab.step(GaitInput::default()).unwrap();
            if lab.outcome != "running" {
                break;
            }
        }
        assert_eq!(lab.outcome, "complete");
        assert!(lab.snapshot().distance_m >= GOAL_M);
        let ticks = lab.ticks;
        lab.step(GaitInput::default()).unwrap();
        assert_eq!(lab.ticks, ticks);
    }
    #[test]
    fn rails_are_collision_geometry_and_change_the_trajectory() {
        let mut flat = Lab::new(Course::Flat);
        let mut rails = Lab::new(Course::Rails);
        assert_eq!(
            rails
                .snapshot()
                .bodies
                .iter()
                .filter(|p| p.kind == "obstacle")
                .count(),
            3
        );
        for _ in 0..240 {
            flat.step(GaitInput::default()).unwrap();
            rails.step(GaitInput::default()).unwrap();
        }
        assert_ne!(flat.snapshot().sample, rails.snapshot().sample);
    }
    #[test]
    fn invalid_inputs_cannot_advance_a_run() {
        let mut lab = Lab::new(Course::Flat);
        for value in [f32::NAN, f32::INFINITY, -1.0, 8.0] {
            assert!(
                lab.step(GaitInput {
                    frequency_hz: value,
                    ..GaitInput::default()
                })
                .is_err()
            );
        }
        assert_eq!(lab.ticks, 0);
    }
    #[test]
    fn request_boundary_rejects_cross_origin_and_rebinding() {
        let mut lab = Lab::new(Course::Flat);
        let body = serde_json::to_string(&GaitInput::default()).unwrap();
        assert!(
            route(
                &mut lab,
                "POST",
                "/api/step",
                ADDRESS,
                Some("https://example.com"),
                &body
            )
            .status
            .starts_with("403")
        );
        assert!(
            route(&mut lab, "GET", "/api/state", "example.com", None, "")
                .status
                .starts_with("403")
        );
        assert_eq!(lab.ticks, 0);
        assert!(
            route(&mut lab, "POST", "/api/step", ADDRESS, Some(ORIGIN), &body)
                .status
                .starts_with("200")
        );
        assert_eq!(lab.ticks, 1);
    }
    #[test]
    fn reset_restores_pose_and_validates_before_mutating() {
        let mut lab = Lab::new(Course::Flat);
        let initial = lab.snapshot().sample;
        lab.step(GaitInput::default()).unwrap();
        assert!(
            route(
                &mut lab,
                "POST",
                "/api/reset",
                ADDRESS,
                Some(ORIGIN),
                "{\"course\":\"invalid\"}"
            )
            .status
            .starts_with("400")
        );
        assert_eq!(lab.ticks, 1);
        assert!(
            route(
                &mut lab,
                "POST",
                "/api/reset",
                ADDRESS,
                Some(ORIGIN),
                "{\"course\":\"flat\"}"
            )
            .status
            .starts_with("200")
        );
        assert_eq!(lab.snapshot().sample, initial);
    }
    #[test]
    fn parser_requires_unique_host_and_json_mutations() {
        let mut lab = Lab::new(Course::Flat);
        assert!(
            dispatch(
                &mut lab,
                "POST /api/step HTTP/1.1\r\nHost: 127.0.0.1:8790\r\n\r\n",
                "{}"
            )
            .status
            .starts_with("415")
        );
        assert!(
            dispatch(
                &mut lab,
                "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:8790\r\nHost: evil\r\n\r\n",
                ""
            )
            .status
            .starts_with("400")
        );
        assert!(
            dispatch(
                &mut lab,
                "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:8790\r\n\r\n",
                ""
            )
            .status
            .starts_with("200")
        );
    }
    #[test]
    fn workshop_routes_are_available_without_advancing_physics() {
        let mut lab = Lab::new(Course::Flat);
        for path in ["/api/workshop", "/api/export", "/api/recordings"] {
            let result = route(&mut lab, "GET", path, ADDRESS, None, "");
            assert_eq!(result.status, "200 OK", "{path}");
        }
        assert_eq!(lab.ticks, 0);
    }
    fn post(lab: &mut Lab, path: &str, body: &impl Serialize) -> Response {
        route(
            lab,
            "POST",
            path,
            ADDRESS,
            Some(ORIGIN),
            &serde_json::to_string(body).unwrap(),
        )
    }
    fn finish_job(lab: &mut Lab) {
        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        while lab.workshop.busy() {
            assert!(
                std::time::Instant::now() < deadline,
                "bounded job timed out"
            );
            lab.workshop.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(lab.workshop.job.status, "complete");
    }
    #[test]
    fn design_evolve_select_save_restore_and_replay_use_one_physical_model() {
        let mut lab = Lab::new(Course::Flat);
        let original = lab
            .snapshot()
            .bodies
            .iter()
            .find(|b| b.kind == "chassis")
            .unwrap()
            .size;
        let design = RobotDesign {
            body_length_m: 0.18,
            ..RobotDesign::default()
        };
        assert_eq!(post(&mut lab, "/api/design", &design).status, "200 OK");
        let changed = lab
            .snapshot()
            .bodies
            .iter()
            .find(|b| b.kind == "chassis")
            .unwrap()
            .size;
        assert_ne!(original, changed);
        assert_eq!(changed[0], 0.09);
        assert_eq!(
            post(
                &mut lab,
                "/api/search",
                &serde_json::json!({"seed":1,"generations":1})
            )
            .status,
            "200 OK"
        );
        assert_eq!(
            post(&mut lab, "/api/design", &design).status,
            "409 Conflict"
        );
        // HTTP state remains readable while the real search works off-thread.
        assert_eq!(
            route(&mut lab, "GET", "/api/state", ADDRESS, None, "").status,
            "200 OK"
        );
        finish_job(&mut lab);
        let elite = lab
            .workshop
            .archive
            .as_ref()
            .unwrap()
            .cells
            .first()
            .expect("real seeded search found a gait")
            .clone();
        assert_eq!(
            post(
                &mut lab,
                "/api/controller",
                &serde_json::json!({"cell":elite.cell})
            )
            .status,
            "200 OK"
        );
        assert_eq!(lab.workshop.controller.as_ref(), Some(&elite.genome));
        let mut independent = FieldSim::new(design.robot().unwrap());
        independent.settle(100);
        let expected = independent.control_step(&elite.genome.actions_at(0.0));
        // Zeroed manual sliders must not erase an evolved controller.
        let actual = lab
            .step(GaitInput {
                frequency_hz: 1.0,
                hip_amplitude: 0.0,
                knee_amplitude: 0.0,
            })
            .unwrap();
        assert_eq!(actual.sample, expected);
        let encoded = route(&mut lab, "GET", "/api/export", ADDRESS, None, "").body;
        let save: Save = serde_json::from_str(&encoded).unwrap();
        assert_eq!(post(&mut lab, "/api/import", &save).status, "200 OK");
        assert_eq!(lab.workshop.controller, Some(elite.genome));
        assert_eq!(lab.workshop.design, design);
        assert_eq!(lab.workshop.archive.as_ref().unwrap().source, "imported");
        for _ in 0..10 {
            lab.step(GaitInput::default()).unwrap();
        }
        assert_eq!(
            post(&mut lab, "/api/recordings/save", &serde_json::json!({})).status,
            "200 OK"
        );
        let id = lab.run_id;
        let sample = lab.snapshot().sample;
        assert_eq!(
            post(
                &mut lab,
                "/api/reset",
                &serde_json::json!({"course":"rails"})
            )
            .status,
            "200 OK"
        );
        let replay = route(
            &mut lab,
            "GET",
            &format!("/api/recording/{id}"),
            ADDRESS,
            None,
            "",
        );
        let value: serde_json::Value = serde_json::from_str(&replay.body).unwrap();
        assert_eq!(value["course"], "flat");
        assert_eq!(value["frames"].as_array().unwrap().len(), 11);
        let replayed: Sample =
            serde_json::from_value(value["frames"][10]["sample"].clone()).unwrap();
        assert_eq!(replayed, sample);
        assert_eq!(
            lab.ticks, 0,
            "reading replay must not advance the live trial"
        );
        assert_eq!(
            post(&mut lab, "/api/holdout", &serde_json::json!({})).status,
            "200 OK"
        );
        finish_job(&mut lab);
        assert_eq!(
            lab.workshop.holdout.as_ref().unwrap().tested,
            lab.workshop.archive.as_ref().unwrap().cells.len()
        );
    }
    #[test]
    fn rejected_import_is_atomic_and_recording_memory_is_bounded() {
        let mut lab = Lab::new(Course::Flat);
        lab.step(GaitInput::default()).unwrap();
        let before = serde_json::to_value(lab.snapshot()).unwrap();
        let mut save = lab.workshop.save(lab.gait);
        save.version = 999;
        assert_eq!(
            post(&mut lab, "/api/import", &save).status,
            "400 Bad Request"
        );
        assert_eq!(serde_json::to_value(lab.snapshot()).unwrap(), before);
        save.version = 1;
        save.controller = Some(crate::CpgGenome::from_gait(&Gait::default()));
        save.controller.as_mut().unwrap().joints.pop();
        assert_eq!(
            post(&mut lab, "/api/import", &save).status,
            "400 Bad Request"
        );
        assert_eq!(serde_json::to_value(lab.snapshot()).unwrap(), before);
        for _ in 0..5 {
            lab.reset_run(Course::Flat);
            lab.step(GaitInput::default()).unwrap();
            lab.save_recording();
        }
        assert_eq!(lab.recordings.len(), 3);
        let latest = lab.recordings.last().unwrap().id;
        lab.save_recording();
        assert_eq!(lab.recordings.len(), 3);
        assert_eq!(lab.recordings.last().unwrap().id, latest);
    }
    #[test]
    fn cancellation_keeps_workshop_available_and_does_not_install_partial_archive() {
        let mut lab = Lab::new(Course::Flat);
        lab.workshop
            .start_search(SearchInput {
                seed: 2,
                generations: 12,
            })
            .unwrap();
        assert!(
            lab.workshop
                .start_search(SearchInput {
                    seed: 3,
                    generations: 1
                })
                .is_err()
        );
        lab.workshop.cancel();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while lab.workshop.busy() {
            assert!(std::time::Instant::now() < deadline);
            lab.workshop.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(lab.workshop.job.status, "cancelled");
        assert!(lab.workshop.archive.is_none());
        assert_eq!(
            post(&mut lab, "/api/design", &RobotDesign::default()).status,
            "200 OK"
        );
    }
}
