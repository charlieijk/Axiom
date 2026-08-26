//! Minimal HTTP server for the replay GUI: asset routes, request dispatch,
//! and the smoke check. The replay API lives in [`replay`].

mod replay;

use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, TrySendError},
    },
    thread,
    time::Duration,
};

use replay::{ReplayRequest, api_replay_response, replay_json};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");
const GRAPHICS_HTML: &str = include_str!("../web/graphics3d.html");
const GRAPHICS_CSS: &str = include_str!("../web/graphics3d.css");
const GRAPHICS_JS: &str = include_str!("../web/graphics3d.js");
const GRAPHICS_STATE_JS: &str = include_str!("../web/graphics3d-state.js");
const GRAPHICS_SCENE_JS: &str = include_str!("../web/graphics3d-scene.js");
const GRAPHICS_JOURNAL_JS: &str = include_str!("../web/graphics3d-journal.js");
const THREE_JS: &str = include_str!("../web/vendor/three.module.min.js");
// Vendored so /3d loads no third-party host. Both carry their font binaries as
// data: URIs, which is what keeps them plain text on this String-only asset
// path; see web/vendor/README.md.
const PHOSPHOR_CSS: &str = include_str!("../web/vendor/phosphor-icons.css");
const FONTS_CSS: &str = include_str!("../web/vendor/fonts.css");

const CONNECTION_WORKERS: usize = 8;
const PENDING_CONNECTIONS: usize = 16;
const MAX_CONCURRENT_REPLAYS: usize = 2;
const REQUEST_IO_TIMEOUT: Duration = Duration::from_secs(5);

struct ReplayLimiter {
    active: AtomicUsize,
    limit: usize,
}

impl ReplayLimiter {
    fn new(limit: usize) -> Self {
        Self {
            active: AtomicUsize::new(0),
            limit,
        }
    }

    fn try_acquire(&self) -> Option<ReplayPermit<'_>> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.limit).then_some(active + 1)
            })
            .ok()
            .map(|_| ReplayPermit { limiter: self })
    }
}

struct ReplayPermit<'a> {
    limiter: &'a ReplayLimiter,
}

impl Drop for ReplayPermit<'_> {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::Release);
    }
}

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

pub fn serve_gui(config: GuiConfig) -> io::Result<SocketAddr> {
    let listener = bind_first_available(&config.host, config.port)?;
    let address = listener.local_addr()?;
    let (sender, receiver) = mpsc::sync_channel(PENDING_CONNECTIONS);
    let receiver = Arc::new(Mutex::new(receiver));
    let replay_limiter = Arc::new(ReplayLimiter::new(MAX_CONCURRENT_REPLAYS));

    start_connection_workers(receiver, replay_limiter)?;

    println!("Axiom GUI running at http://{address}");
    println!("Press Ctrl-C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = configure_connection(&stream, REQUEST_IO_TIMEOUT) {
                    eprintln!("failed to configure connection deadlines: {error}");
                    continue;
                }

                match sender.try_send(stream) {
                    Ok(()) => {}
                    Err(TrySendError::Full(mut stream)) => {
                        let _ = write_response(
                            &mut stream,
                            "503 Service Unavailable",
                            "text/plain; charset=utf-8",
                            "Server is busy; retry later.",
                            Some(("Retry-After", "1")),
                        );
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        return Err(io::Error::other("all GUI connection workers stopped"));
                    }
                }
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }

    Ok(address)
}

fn start_connection_workers(
    receiver: Arc<Mutex<Receiver<TcpStream>>>,
    replay_limiter: Arc<ReplayLimiter>,
) -> io::Result<()> {
    for worker_index in 0..CONNECTION_WORKERS {
        let receiver = Arc::clone(&receiver);
        let replay_limiter = Arc::clone(&replay_limiter);
        thread::Builder::new()
            .name(format!("axiom-gui-request-{worker_index}"))
            .spawn(move || connection_worker(&receiver, &replay_limiter))?;
    }
    Ok(())
}

fn connection_worker(receiver: &Mutex<Receiver<TcpStream>>, replay_limiter: &ReplayLimiter) {
    loop {
        let stream = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        let Ok(stream) = stream else {
            return;
        };
        let peer = stream.peer_addr().ok();
        if let Err(error) = handle_connection(stream, replay_limiter) {
            match peer {
                Some(peer) => eprintln!("request from {peer} failed: {error}"),
                None => eprintln!("request failed: {error}"),
            }
        }
    }
}

fn configure_connection(stream: &TcpStream, timeout: Duration) -> io::Result<()> {
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))
}

pub fn gui_smoke_check() -> io::Result<()> {
    let html = response_body("/");
    let css = response_body("/styles.css");
    let js = response_body("/app.js");
    let graphics = response_body("/3d");
    let graphics_js = response_body("/graphics3d.js");
    let three_js = response_body("/vendor/three.module.min.js");
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
    if !graphics.contains("graphics-canvas")
        || !graphics_js.contains("/vendor/three.module.min.js")
        || !three_js.contains("const t=\"165\"")
    {
        return Err(io::Error::other(
            "3D GUI assets did not include expected scene markers",
        ));
    }
    let phosphor_css = response_body("/vendor/phosphor-icons.css");
    let fonts_css = response_body("/vendor/fonts.css");
    if !phosphor_css.contains(".ph-pause:before") || !fonts_css.contains("Manrope Variable") {
        return Err(io::Error::other(
            "vendored icon and font stylesheets were not served",
        ));
    }
    // /3d must stay self-hosted: an external <link> or <script> is a silent
    // dependency on a host Axiom does not control.
    if graphics.contains("https://") {
        return Err(io::Error::other(
            "3D GUI page referenced an external host; vendor the asset instead",
        ));
    }
    let graphics_state_js = response_body("/graphics3d-state.js");
    let graphics_scene_js = response_body("/graphics3d-scene.js");
    let graphics_journal_js = response_body("/graphics3d-journal.js");
    if !graphics_state_js.contains("replayRequest")
        || !graphics_scene_js.contains("createCreatureRenderer")
        || !graphics_journal_js.contains("renderFieldJournal")
    {
        return Err(io::Error::other(
            "3D GUI modules did not include expected module markers",
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

fn handle_connection(mut stream: TcpStream, replay_limiter: &ReplayLimiter) -> io::Result<()> {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, content_type, body, extra_header) = match path {
        "/" | "/index.html" => (
            "200 OK",
            "text/html; charset=utf-8",
            response_body("/"),
            None,
        ),
        "/3d" | "/3d.html" => (
            "200 OK",
            "text/html; charset=utf-8",
            response_body("/3d"),
            None,
        ),
        "/styles.css" => (
            "200 OK",
            "text/css; charset=utf-8",
            response_body(path),
            None,
        ),
        "/graphics3d.css" => (
            "200 OK",
            "text/css; charset=utf-8",
            response_body(path),
            None,
        ),
        "/app.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
            None,
        ),
        "/graphics3d.js"
        | "/graphics3d-state.js"
        | "/graphics3d-scene.js"
        | "/graphics3d-journal.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
            None,
        ),
        "/vendor/three.module.min.js" => (
            "200 OK",
            "application/javascript; charset=utf-8",
            response_body(path),
            None,
        ),
        "/vendor/phosphor-icons.css" | "/vendor/fonts.css" => (
            "200 OK",
            "text/css; charset=utf-8",
            response_body(path),
            None,
        ),
        "/api/replay" => {
            let Some(_permit) = replay_limiter.try_acquire() else {
                return write_response(
                    &mut stream,
                    "503 Service Unavailable",
                    "application/json; charset=utf-8",
                    r#"{"error":"replay capacity is busy; retry later"}"#,
                    Some(("Retry-After", "1")),
                );
            };
            let (status, content_type, body) = api_replay_response(query);
            (status, content_type, body, None)
        }
        _ => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found".to_string(),
            None,
        ),
    };

    write_response(&mut stream, status, content_type, &body, extra_header)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
    extra_header: Option<(&str, &str)>,
) -> io::Result<()> {
    let extra_header = extra_header
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .unwrap_or_default();
    let response = format!(
        // `no-store` is why the pages carry no `?v=` cache-busting query
        // strings: there is no cache here to bust, and a hand-maintained
        // version token would only drift from the file it claims to describe.
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{extra_header}Connection: close\r\n\r\n{body}",
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
        "/graphics3d-state.js" => GRAPHICS_STATE_JS.to_string(),
        "/graphics3d-scene.js" => GRAPHICS_SCENE_JS.to_string(),
        "/graphics3d-journal.js" => GRAPHICS_JOURNAL_JS.to_string(),
        "/vendor/three.module.min.js" => THREE_JS.to_string(),
        "/vendor/phosphor-icons.css" => PHOSPHOR_CSS.to_string(),
        "/vendor/fonts.css" => FONTS_CSS.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Read, Write},
        net::{Shutdown, TcpListener, TcpStream},
        time::{Duration, Instant},
    };

    use super::{
        ReplayLimiter, bind_first_available, configure_connection, gui_smoke_check,
        handle_connection,
    };

    #[test]
    fn gui_smoke_check_loads_assets_and_replay_data() {
        gui_smoke_check().expect("GUI smoke check should pass");
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

    #[test]
    fn replay_admission_is_bounded_and_releases_on_drop() {
        let limiter = ReplayLimiter::new(1);
        let permit = limiter
            .try_acquire()
            .expect("first replay should be admitted");

        assert!(limiter.try_acquire().is_none());
        drop(permit);
        assert!(limiter.try_acquire().is_some());
    }

    #[test]
    fn replay_overload_returns_service_unavailable_without_running_replay() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let mut client =
            TcpStream::connect(listener.local_addr().expect("listener has an address"))
                .expect("test client should connect");
        let (server, _) = listener.accept().expect("server should accept test client");
        configure_connection(&server, Duration::from_secs(1))
            .expect("test connection should accept deadlines");
        let limiter = ReplayLimiter::new(1);
        let _permit = limiter
            .try_acquire()
            .expect("test should occupy replay capacity");
        client
            .write_all(b"GET /api/replay?frames=1 HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("test request should write");
        client
            .shutdown(Shutdown::Write)
            .expect("test request should finish");

        handle_connection(server, &limiter).expect("overload response should write");
        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .expect("test response should read");

        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
        assert!(response.contains("Retry-After: 1\r\n"));
        assert!(response.contains("replay capacity is busy"));
    }

    #[test]
    fn idle_connection_read_is_bounded_by_its_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
        let client = TcpStream::connect(listener.local_addr().expect("listener has an address"))
            .expect("test client should connect");
        let (server, _) = listener.accept().expect("server should accept test client");
        configure_connection(&server, Duration::from_millis(50))
            .expect("test connection should accept deadlines");

        let started = Instant::now();
        let error = handle_connection(server, &ReplayLimiter::new(1))
            .expect_err("an idle connection must time out");

        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(client);
    }
}
