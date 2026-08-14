#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SITE_DIR="$ROOT_DIR/site"

[[ -f "$SITE_DIR/package.json" ]] || {
  echo "Missing canonical site manifest: $SITE_DIR/package.json" >&2
  exit 1
}
[[ -f "$SITE_DIR/package-lock.json" ]] || {
  echo "Missing canonical site lockfile: $SITE_DIR/package-lock.json" >&2
  exit 1
}

npm --prefix "$SITE_DIR" ci
npm --prefix "$SITE_DIR" test
