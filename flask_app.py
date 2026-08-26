from __future__ import annotations

import atexit
import os
import signal
import socket
import subprocess
import time
from pathlib import Path
from urllib.parse import urlsplit

import requests
from flask import Flask, Response, jsonify, request, stream_with_context


ROOT = Path(__file__).resolve().parent
DEFAULT_BACKEND_URL = "http://127.0.0.1:58110"
HOP_BY_HOP_HEADERS = {
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
}
PROXY_METHODS = ["GET", "HEAD"]
HTTP = requests.Session()
HTTP.trust_env = False
LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1"}


def _flask_host() -> str:
    host = os.environ.get("FLASK_HOST", "127.0.0.1")
    if host not in LOOPBACK_HOSTS:
        raise RuntimeError("The Flask companion may only bind to a loopback host.")
    return host


def _is_loopback_http(url: str) -> bool:
    parsed = urlsplit(url)
    return (
        parsed.scheme == "http"
        and parsed.hostname in {"127.0.0.1", "localhost", "::1"}
        and parsed.port is not None
    )


def _reachable(url: str) -> bool:
    parsed = urlsplit(url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 80
    try:
        with socket.create_connection((host, port), timeout=0.25):
            return True
    except OSError:
        return False


def _compatible_backend(url: str) -> bool:
    try:
        response = HTTP.get(
            f"{url.rstrip('/')}/api/replay?frames=1",
            allow_redirects=False,
            timeout=1,
        )
        payload = response.json()
    except (requests.RequestException, ValueError):
        return False
    return (
        response.status_code == 200
        and isinstance(payload, dict)
        and isinstance(payload.get("frames"), list)
        and "body" in payload
    )


def _stop_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
        process.wait(timeout=5)
    except ProcessLookupError:
        return
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        process.wait(timeout=5)


def _wait_for_backend(
    process: subprocess.Popen[bytes],
    url: str,
    timeout: float = 120,
) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Axiom backend exited with status {process.returncode}.")
        if _compatible_backend(url):
            return
        time.sleep(0.1)
    raise RuntimeError(f"Timed out waiting for the Axiom backend at {url}.")


def start_backend(url: str = DEFAULT_BACKEND_URL) -> subprocess.Popen[bytes] | None:
    """Start Axiom's existing Rust GUI server on a private loopback port."""
    upstream = url.rstrip("/")
    if not _is_loopback_http(upstream):
        raise RuntimeError("Automatic Axiom startup only supports a loopback HTTP backend with an explicit port.")
    if _reachable(upstream):
        if _compatible_backend(upstream):
            return None
        raise RuntimeError(f"Port for {upstream} is occupied by an incompatible service.")

    parsed = urlsplit(upstream)
    process = subprocess.Popen(
        [
            "cargo",
            "run",
            "--",
            "gui",
            "--host",
            "127.0.0.1",
            "--port",
            str(parsed.port),
        ],
        cwd=ROOT,
        start_new_session=(os.name == "posix"),
    )
    try:
        _wait_for_backend(process, upstream)
    except BaseException:
        _stop_process(process)
        raise
    atexit.register(_stop_process, process)
    return process


def _request_headers() -> dict[str, str]:
    connection_headers = {
        item.strip().lower()
        for item in request.headers.get("Connection", "").split(",")
        if item.strip()
    }
    excluded = HOP_BY_HOP_HEADERS | connection_headers | {"host", "content-length"}
    return {
        key: value
        for key, value in request.headers
        if key.lower() not in excluded
    }


def _response_headers(response: requests.Response) -> list[tuple[str, str]]:
    connection_headers = {
        item.strip().lower()
        for item in response.headers.get("Connection", "").split(",")
        if item.strip()
    }
    excluded = HOP_BY_HOP_HEADERS | connection_headers
    headers: list[tuple[str, str]] = []
    for key in response.raw.headers:
        if key.lower() in excluded:
            continue
        for value in response.raw.headers.getlist(key):
            headers.append((key, value))
    return headers


def _request_has_body() -> bool:
    """Detect a proxy body while reading at most one byte from an unframed stream."""
    content_length = request.headers.get("Content-Length")
    if content_length is not None:
        try:
            return int(content_length) != 0
        except ValueError:
            return True
    if "Transfer-Encoding" in request.headers:
        return True
    return bool(request.stream.read(1))


def create_app(backend_url: str | None = None) -> Flask:
    """Create a Flask reverse proxy without replacing Axiom's Rust authority."""
    upstream = (
        backend_url or os.environ.get("AXIOM_BACKEND_URL", DEFAULT_BACKEND_URL)
    ).rstrip("/")
    app = Flask(__name__, static_folder=None)

    @app.get("/healthz")
    def healthz():
        return jsonify(
            app="axiom",
            backend_ready=_compatible_backend(upstream),
            backend_url=upstream,
            mode="rust-proxy",
            status="ok",
        )

    @app.route(
        "/",
        defaults={"proxy_path": ""},
        methods=PROXY_METHODS,
        provide_automatic_options=False,
    )
    @app.route(
        "/<path:proxy_path>",
        methods=PROXY_METHODS,
        provide_automatic_options=False,
    )
    def proxy(proxy_path: str):
        if _request_has_body():
            return jsonify(error="Proxy requests must not include a body."), 413

        target = upstream + "/" + proxy_path
        if request.query_string:
            target += "?" + request.query_string.decode("latin-1")
        proxy_session = requests.Session()
        proxy_session.trust_env = False
        try:
            upstream_response = proxy_session.request(
                method=request.method,
                url=target,
                headers=_request_headers(),
                allow_redirects=False,
                stream=True,
                timeout=(5, None),
            )
        except requests.RequestException as error:
            proxy_session.close()
            return jsonify(error="Axiom backend is unavailable.", detail=str(error)), 502

        @stream_with_context
        def body():
            try:
                yield from upstream_response.raw.stream(64 * 1024, decode_content=False)
            finally:
                upstream_response.close()
                proxy_session.close()

        proxy_response = Response(
            body(),
            status=upstream_response.status_code,
            headers=_response_headers(upstream_response),
        )
        proxy_response.call_on_close(upstream_response.close)
        proxy_response.call_on_close(proxy_session.close)
        return proxy_response

    return app


app = create_app()


if __name__ == "__main__":
    backend_url = os.environ.get("AXIOM_BACKEND_URL", DEFAULT_BACKEND_URL).rstrip("/")
    previous_sigterm = signal.getsignal(signal.SIGTERM)

    def handle_sigterm(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, handle_sigterm)
    process: subprocess.Popen[bytes] | None = None
    try:
        process = start_backend(backend_url)
        runtime_app = create_app(backend_url=backend_url)
        runtime_app.run(
            host=_flask_host(),
            port=int(os.environ.get("PORT", "5110")),
            debug=False,
            threaded=True,
        )
    finally:
        if process is not None:
            _stop_process(process)
        signal.signal(signal.SIGTERM, previous_sigterm)
