# Apple Silicon source build

Axiom runs natively on Apple Silicon as an `aarch64-apple-darwin` command-line
binary and opens its interactive interface through a loopback-only local web
server. It is not a conventional `.app` bundle, and no prebuilt challenge
binary is currently published.

## Build and run

Prerequisites: Git and Rust 1.85 or newer on an Apple Silicon Mac.

```sh
git clone https://github.com/charlieijk/Axiom.git
cd Axiom
uname -m
cargo test --locked --test arm64_evidence
cargo run --release --locked --bin axiom -- gui
```

`uname -m` should print `arm64`. Open the printed address with `/3d` appended,
normally `http://127.0.0.1:8787/3d`. If port 8787 is occupied, Axiom probes the
next 19 ports and prints the one it selected. Press Control-C in the terminal
to stop the server.

The 2D interface and the Three.js-powered 3D Field Journal are embedded and
work offline. The 3D page requests cosmetic fonts and icons from public CDNs,
but its controls, text, simulation, and rendering do not depend on them.

## Evidence-oriented check

The full source-backed reviewer path, expected digests, raw allocation reports,
and limitations are documented in
[ARM64_OPTIMIZATION.md](ARM64_OPTIMIZATION.md#arm64-judge-quick-check). The
repository also includes the MIT license, the vendored Three.js license, and
the raw baseline, optimized, and comparison JSON under `docs/benchmarks/`.

The compiled executable embeds Axiom's Rust replay server and 2D/3D browser
clients. It does not bundle checkpoints, credentials, or user data.
