#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )); then
  echo "usage: $0 [--build-only|--debug|--logs|--telemetry|--verify]" >&2
  exit 2
fi
MODE="${1:-run}"
case "$MODE" in
  run|--build-only|build-only|--debug|debug|--logs|logs|--telemetry|telemetry|--verify|verify) ;;
  --help|-h|help)
    echo "usage: $0 [--build-only|--debug|--logs|--telemetry|--verify]"
    echo "Axiom is a loopback Rust service with a browser GUI; it is not a native macOS app."
    exit 0
    ;;
  *)
    echo "usage: $0 [--build-only|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
APP_BINARY="$CARGO_TARGET_DIR/debug/axiom"
APP_URL="${AXIOM_URL:-http://127.0.0.1:8787}"
RUNTIME_DIR="${TMPDIR:-/tmp}/axiom-codex-${UID}"
PID_FILE="$RUNTIME_DIR/service.pid"
LOG_FILE="$RUNTIME_DIR/service.log"
mkdir -p "$RUNTIME_DIR"
cd "$ROOT_DIR"

build() {
  cargo build --locked --bin axiom
}

stop_existing() {
  [[ -f "$PID_FILE" ]] || return 0
  local pid command
  pid="$(cat "$PID_FILE")"
  if [[ ! "$pid" =~ ^[0-9]+$ ]] || ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$PID_FILE"
    return 0
  fi
  command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  if [[ "$command" != *"$APP_BINARY"* ]]; then
    echo "Refusing to stop PID $pid because it is not the Axiom process recorded by this script." >&2
    exit 1
  fi
  kill "$pid"
  for _ in {1..30}; do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$pid" 2>/dev/null || true
  rm -f "$PID_FILE"
}

launch() {
  : >"$LOG_FILE"
  nohup "$APP_BINARY" gui >>"$LOG_FILE" 2>&1 </dev/null &
  local pid=$!
  printf '%s\n' "$pid" >"$PID_FILE"
  echo "Axiom browser GUI starting at $APP_URL (PID $pid; log $LOG_FILE)."
  if [[ "${CODEX_OPEN_BROWSER:-0}" == "1" ]]; then
    /usr/bin/open "$APP_URL"
  fi
}

verify() {
  local pid
  pid="$(cat "$PID_FILE")"
  for _ in {1..30}; do
    if kill -0 "$pid" 2>/dev/null && curl --fail --silent --output /dev/null "$APP_URL"; then
      echo "Verified Axiom PID $pid at $APP_URL."
      return 0
    fi
    sleep 1
  done
  echo "Axiom did not become ready; inspect $LOG_FILE." >&2
  return 1
}

if [[ "$MODE" == "--build-only" || "$MODE" == "build-only" ]]; then
  build
  exit 0
fi

stop_existing
build

case "$MODE" in
  run)
    launch
    ;;
  --debug|debug)
    if command -v rust-lldb >/dev/null 2>&1; then
      exec rust-lldb -- "$APP_BINARY" gui
    fi
    exec lldb -- "$APP_BINARY" gui
    ;;
  --logs|logs)
    launch
    exec tail -n 100 -F "$LOG_FILE"
    ;;
  --telemetry|telemetry)
    echo "Axiom has no dedicated macOS telemetry subsystem; streaming its service log."
    launch
    exec tail -n 100 -F "$LOG_FILE"
    ;;
  --verify|verify)
    launch
    verify
    ;;
esac
