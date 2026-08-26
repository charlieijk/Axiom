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

stat_owner() {
  if stat -f '%u' "$1" 2>/dev/null; then
    return
  fi
  stat -c '%u' "$1"
}

stat_mode() {
  if stat -f '%Lp' "$1" 2>/dev/null; then
    return
  fi
  stat -c '%a' "$1"
}

stat_links() {
  if stat -f '%l' "$1" 2>/dev/null; then
    return
  fi
  stat -c '%h' "$1"
}

validate_runtime_file() {
  local path="$1"
  local label="$2"
  [[ -L "$path" ]] && {
    echo "Refusing to use symlinked Axiom $label: $path" >&2
    exit 1
  }
  [[ -e "$path" ]] || return 0
  [[ -f "$path" ]] || {
    echo "Refusing to use non-regular Axiom $label: $path" >&2
    exit 1
  }
  [[ "$(stat_owner "$path")" == "$(id -u)" ]] || {
    echo "Refusing to use Axiom $label owned by another user: $path" >&2
    exit 1
  }
  [[ "$(stat_links "$path")" == "1" ]] || {
    echo "Refusing to use multiply linked Axiom $label: $path" >&2
    exit 1
  }
  chmod 600 "$path"
  [[ "$(stat_mode "$path")" == "600" ]] || {
    echo "Could not make Axiom $label private: $path" >&2
    exit 1
  }
}

secure_runtime_dir() {
  local previous_umask
  [[ -L "$RUNTIME_DIR" ]] && {
    echo "Refusing to use symlinked Axiom runtime directory: $RUNTIME_DIR" >&2
    exit 1
  }
  if [[ ! -e "$RUNTIME_DIR" ]]; then
    previous_umask="$(umask)"
    umask 077
    mkdir "$RUNTIME_DIR"
    umask "$previous_umask"
  fi
  [[ -d "$RUNTIME_DIR" && ! -L "$RUNTIME_DIR" ]] || {
    echo "Refusing to use non-directory Axiom runtime path: $RUNTIME_DIR" >&2
    exit 1
  }
  [[ "$(stat_owner "$RUNTIME_DIR")" == "$(id -u)" ]] || {
    echo "Refusing to use Axiom runtime directory owned by another user: $RUNTIME_DIR" >&2
    exit 1
  }
  chmod 700 "$RUNTIME_DIR"
  [[ "$(stat_mode "$RUNTIME_DIR")" == "700" ]] || {
    echo "Could not make Axiom runtime directory private: $RUNTIME_DIR" >&2
    exit 1
  }
  validate_runtime_file "$PID_FILE" "PID file"
  validate_runtime_file "$LOG_FILE" "log file"
}

secure_runtime_dir
cd "$ROOT_DIR"

build() {
  cargo build --locked --bin axiom
}

stop_existing() {
  validate_runtime_file "$PID_FILE" "PID file"
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
  local previous_umask pid
  validate_runtime_file "$LOG_FILE" "log file"
  validate_runtime_file "$PID_FILE" "PID file"
  previous_umask="$(umask)"
  umask 077
  : >"$LOG_FILE"
  nohup "$APP_BINARY" gui >>"$LOG_FILE" 2>&1 </dev/null &
  pid=$!
  printf '%s\n' "$pid" >"$PID_FILE"
  chmod 600 "$LOG_FILE" "$PID_FILE"
  umask "$previous_umask"
  echo "Axiom browser GUI starting at $APP_URL (PID $pid; log $LOG_FILE)."
  if [[ "${CODEX_OPEN_BROWSER:-0}" == "1" ]]; then
    /usr/bin/open "$APP_URL"
  fi
}

verify() {
  local pid
  validate_runtime_file "$PID_FILE" "PID file"
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
