# Apple Silicon test build

The challenge release provides a native `aarch64-apple-darwin` command-line
binary for Apple Silicon Macs. Axiom opens its interactive interface through a
loopback-only local web server; it is not a conventional `.app` bundle.

## Downloaded build

Download the adjacent `.sha256` release asset, verify it, then extract the
archive:

```sh
shasum -a 256 -c axiom-arm-create-2026-aarch64-apple-darwin.tar.gz.sha256
tar -xzf axiom-arm-create-2026-aarch64-apple-darwin.tar.gz
cd axiom-arm-create-2026-aarch64-apple-darwin
./axiom gui
```

Open the printed address with `/3d` appended, normally
`http://127.0.0.1:8787/3d`. If port 8787 is occupied, Axiom probes the next 19
ports and prints the one it selected. Press Control-C in the terminal to stop
the server.

The preview binary is ad-hoc signed, not Apple-notarized. macOS may require an
explicit approval in **System Settings → Privacy & Security** after the first
launch. If organizational policy blocks unnotarized, ad-hoc-signed preview
binaries, use the source-build path below.

The 2D interface and the Three.js-powered 3D Field Journal are embedded and
work offline. The 3D page requests cosmetic fonts and icons from public CDNs,
but its controls, text, simulation, and rendering do not depend on them.

## Source-build fallback

Prerequisites: Git and Rust 1.85 or newer on an Apple Silicon Mac.

```sh
git clone --branch arm-create-2026 https://github.com/charlieijk/Axiom.git
cd Axiom
uname -m
cargo run --release --locked --bin axiom -- gui
```

`uname -m` should print `arm64`. For the full evidence-oriented reviewer path,
including exact expected digests, see the tag-pinned
[Arm64 optimization protocol](https://github.com/charlieijk/Axiom/blob/arm-create-2026/docs/ARM64_OPTIMIZATION.md#arm64-judge-quick-check).
The same document is included beside this guide in the downloaded archive.

## Included files

- `axiom` — native Apple Silicon executable
- `LICENSE` — MIT license
- `APPLE_SILICON.md` — this guide
- `ARM64_OPTIMIZATION.md` — measured protocol, results, and limitations
- `THREE-LICENSE.txt` — license for the embedded Three.js r165 module
- `benchmarks/` — raw baseline, optimized, and comparison JSON

The executable embeds Axiom's Rust replay server and 2D/3D browser clients. It
does not bundle checkpoints, credentials, or user data.
