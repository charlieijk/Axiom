# Axiom

Axiom is a Rust embodied-evolution sandbox. It evolves simple body plans and neural controllers across terrain tasks, then keeps diverse elites in a MAP-Elites archive instead of collapsing everything to one winner.

Start with the reviewer-ready walkthrough in [DEMO.md](DEMO.md).

## Arm64 Optimization Challenge

Axiom is entered in **Track 1 — Physical AI**: simulated sensor observations
drive neural-controller inference, actuator commands, embodied motion, fitness,
and MAP-Elites selection in one deterministic loop.

Axiom's deterministic rollout hot path now reuses observation, neural-network,
action, and snapshot buffers. On an Apple M2 Arm64 host, the full fixed-seed
workload used **94.38% fewer allocation/reallocation calls** and requested
**89.73% fewer allocation bytes**, while the complete evolution report remained
identical to the pre-optimization baseline. See the reproducible protocol, raw
JSON, source hashes, and limitations in
[docs/ARM64_OPTIMIZATION.md](docs/ARM64_OPTIMIZATION.md). The curated screenshot
sequence and sub-three-minute recording plan are in
[docs/SUBMISSION_ASSETS.md](docs/SUBMISSION_ASSETS.md).

Arm64 judges can run the copy-paste quick verification and inspect its expected
semantic digest in the
[Arm64 judge quick check](docs/ARM64_OPTIMIZATION.md#arm64-judge-quick-check).
Apple Silicon release usage and the source-build fallback are documented in
[docs/APPLE_SILICON.md](docs/APPLE_SILICON.md).

![Axiom's measured Apple M2 Arm64 allocation result](docs/axiom-arm64-result.png)

![Axiom Archive Lab showing a live evolved creature and selectable MAP-Elites repertoire](docs/axiom-archive-lab-gen12.jpg)

This rebuild is scoped from the previous Axiom review thread:

- morphology genomes with directional attachments
- compiled neural controllers with explicit bias handling
- feedforward, recurrent, and CPG controller modes
- stateful rollout semantics shared by evaluation and playback-style code
- deterministic simulation with replace-on-respawn behavior
- task fitness for flat and rough terrain
- MAP-Elites quality-diversity search
- regression tests for the correctness failures called out in review

## Quick Start

```sh
cargo test --workspace
cargo run -- demo
cargo run -- gui
cargo run -- animate cpg
cargo run -- evaluate cpg
cargo run -- evolve --generations 10 --population 32 --steps 180 --save checkpoints/run.json
cargo run -- inspect checkpoints/run.json
cargo run -- animate --checkpoint checkpoints/run.json --frames 160 --fps 20
cargo run --example rough_terrain_benchmark
cargo run --release --locked --example arm64_optimization_benchmark -- --quick
```

`cargo run -- gui` binds `127.0.0.1:8787` and is unauthenticated by intent —
a single-user local tool. Do not expose it beyond the host; see
[SECURITY.md](SECURITY.md) for the trust boundary.

## Architecture

See the judge-readable system map and authority boundaries in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

- `genome.rs` encodes body morphology and neural genomes.
- `network.rs` compiles neural genomes and keeps bias nodes separate from external inputs.
- `policy.rs` implements feedforward, recurrent, and CPG brain execution.
- `simulation.rs` owns the deterministic physics approximation and body attachment geometry.
- `fitness.rs` defines task rollout and the shared observation vector.
- `qd.rs` implements MAP-Elites archive insertion, replacement, and sampling.
- `evolution.rs` ties evaluation, archive maintenance, and parent selection together.
- `checkpoint.rs` saves complete, versioned experiment state with atomic replacement.

## Current Scope

The Rust core is complete and covered by regression tests, and a browser surface now sits on top of it: a 2D creature simulator at `/`, a 3D evolved-stride viewer at `/3d` (orbit camera, follow mode, terrain presets, and a selectable MAP-Elites Archive Lab; Three.js r165 is bundled for offline use), and a `/api/replay` endpoint that serves both minimal random genomes and `mode=evolved` replays evolved on demand. Evolved replays now carry the selected genome's real parent lineage and mutation record rather than inferring ancestry from generation aggregates.

## Reproducible Experiments

Save the full configuration, champion, archive, generation history, and lineage in a versioned checkpoint:

```sh
cargo run -- evolve --generations 16 --population 32 --steps 180 --seed 42 --save checkpoints/run-42.json
cargo run -- inspect checkpoints/run-42.json
cargo run -- animate --checkpoint checkpoints/run-42.json --frames 160 --fps 20
```

Checkpoint writes use a temporary sibling file and atomic replacement, so an interrupted save does not leave a partially written destination. Unknown checkpoint versions and malformed files fail with an explicit load error.

The `animate` command provides a terminal replay renderer over the same simulation loop:

```sh
cargo run -- animate cpg --task rough --frames 160 --fps 20
cargo run -- animate feedforward --task flat --frames 80 --fps 30
```

The `gui` command starts a local browser interface backed by Rust replay data:

```sh
cargo run -- gui
open http://127.0.0.1:8787
```

## License

MIT — see [LICENSE](LICENSE).
