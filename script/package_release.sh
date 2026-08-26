#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <binary> <vMAJOR.MINOR.PATCH> <target> <output-directory>" >&2
}

if (( $# != 4 )); then
  usage
  exit 2
fi

BINARY_PATH="$1"
RELEASE_VERSION="$2"
TARGET_NAME="$3"
OUTPUT_DIR="$4"

if [[ ! "$RELEASE_VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "release version must be a stable semantic version such as v0.1.1" >&2
  exit 2
fi
if [[ ! "$TARGET_NAME" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "target may contain only letters, numbers, dots, underscores, and hyphens" >&2
  exit 2
fi
if [[ ! -f "$BINARY_PATH" || ! -x "$BINARY_PATH" ]]; then
  echo "release binary must be an executable file: $BINARY_PATH" >&2
  exit 2
fi

EXPECTED_VERSION="axiom ${RELEASE_VERSION#v}"
if ! REPORTED_VERSION="$("$BINARY_PATH" --version)"; then
  echo "release binary did not report its version" >&2
  exit 1
fi
if [[ "$REPORTED_VERSION" != "$EXPECTED_VERSION" ]]; then
  echo "release binary reported '$REPORTED_VERSION'; expected '$EXPECTED_VERSION'" >&2
  exit 1
fi

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_FILES=(
  "README.md"
  "SECURITY.md"
  "LICENSE"
  "docs/APPLE_SILICON.md"
  "web/vendor/THREE-LICENSE.txt"
  "web/vendor/MANROPE-LICENSE.txt"
  "web/vendor/NEWSREADER-LICENSE.txt"
  "web/vendor/PHOSPHOR-LICENSE.txt"
)
for relative_path in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "$PROJECT_ROOT/$relative_path" ]]; then
    echo "required release file is missing: $relative_path" >&2
    exit 1
  fi
done

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"
ARCHIVE_NAME="axiom-${RELEASE_VERSION}-${TARGET_NAME}"
ARCHIVE_PATH="$OUTPUT_DIR/$ARCHIVE_NAME.tar.gz"
CHECKSUM_PATH="$ARCHIVE_PATH.sha256"
if [[ -e "$ARCHIVE_PATH" || -e "$CHECKSUM_PATH" ]]; then
  echo "refusing to overwrite an existing release artifact for $ARCHIVE_NAME" >&2
  exit 1
fi

STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/axiom-release.XXXXXX")"
trap 'rm -rf "$STAGING_ROOT"' EXIT
STAGING_DIR="$STAGING_ROOT/$ARCHIVE_NAME"
mkdir -p "$STAGING_DIR/THIRD_PARTY_LICENSES"

install -m 0755 "$BINARY_PATH" "$STAGING_DIR/axiom"
install -m 0644 "$PROJECT_ROOT/README.md" "$STAGING_DIR/README.md"
install -m 0644 "$PROJECT_ROOT/SECURITY.md" "$STAGING_DIR/SECURITY.md"
install -m 0644 "$PROJECT_ROOT/LICENSE" "$STAGING_DIR/LICENSE"
install -m 0644 "$PROJECT_ROOT/docs/APPLE_SILICON.md" "$STAGING_DIR/INSTALL.md"
for license_path in "$PROJECT_ROOT"/web/vendor/*-LICENSE.txt; do
  install -m 0644 "$license_path" "$STAGING_DIR/THIRD_PARTY_LICENSES/$(basename "$license_path")"
done

COPYFILE_DISABLE=1 tar -C "$STAGING_ROOT" -czf "$ARCHIVE_PATH" "$ARCHIVE_NAME"
if command -v sha256sum >/dev/null 2>&1; then
  CHECKSUM="$(sha256sum "$ARCHIVE_PATH" | awk '{print $1}')"
else
  CHECKSUM="$(shasum -a 256 "$ARCHIVE_PATH" | awk '{print $1}')"
fi
printf '%s  %s\n' "$CHECKSUM" "$(basename "$ARCHIVE_PATH")" > "$CHECKSUM_PATH"

printf 'created %s\ncreated %s\n' "$ARCHIVE_PATH" "$CHECKSUM_PATH"
