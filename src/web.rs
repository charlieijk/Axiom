//! Minimal HTTP server for the replay GUI: asset routes, request dispatch,
//! and the smoke check. The replay API lives in [`replay`].

mod replay;

use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
};

use replay::{ReplayRequest, api_replay_response, replay_json};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_CSS: &str = include_str!("../web/styles.css");
const APP_JS: &str = include_str!("../web/app.js");
const GRAPHICS_HTML: &str = include_str!("../web/graphics3d.html");
const GRAPHICS_CSS: &str = include_str!("../web/graphics3d.css");
const GRAPHICS_JS: &str = include_str!("../web/graphics3d.js");

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

#[cfg(test)]
mod tests {
    use super::{bind_first_available, gui_smoke_check};

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
}
