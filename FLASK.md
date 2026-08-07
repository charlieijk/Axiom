# Flask compatibility

The Flask entry point is a loopback reverse proxy for Axiom's existing Rust
GUI server. Rust remains authoritative for replay generation and evolution;
the adapter does not reimplement those paths in Python.

## Start

From the repository root, using a Python environment with the dependencies in
`requirements-flask.txt`:

```bash
python flask_app.py
```

Open <http://127.0.0.1:5110>. Set `PORT` to change the public Flask port and
`FLASK_HOST` to change its bind address. The safe default is loopback.

When run directly, Flask supervises this private backend command:

```bash
cargo run -- gui --host 127.0.0.1 --port 58110
```

`AXIOM_BACKEND_URL` may select another existing loopback Axiom server. An
occupied backend port is reused only when it answers Axiom's replay contract;
an unrelated service causes startup to fail.

## Readiness

`GET /healthz` reports whether the Rust replay endpoint is ready. All other
paths, including `/`, `/3d`, assets, and `/api/replay`, stream through to the
Rust server.

Importing `flask_app:app` creates the WSGI application but does not spawn a
process. A WSGI runner must start the Rust server separately, or set
`AXIOM_BACKEND_URL` to one already running.
