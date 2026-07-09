# Axiom

Axiom is a Rust embodied-evolution sandbox. It evolves simple body plans and neural controllers across terrain tasks, then keeps diverse elites in a MAP-Elites archive instead of collapsing everything to one winner.

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
cargo run -- evolve --generations 10 --population 32 --steps 180
cargo run --example rough_terrain_benchmark
```

## Architecture

- `genome.rs` encodes body morphology and neural genomes.
- `network.rs` compiles neural genomes and keeps bias nodes separate from external inputs.
- `policy.rs` implements feedforward, recurrent, and CPG brain execution.
- `simulation.rs` owns the deterministic physics approximation and body attachment geometry.
- `fitness.rs` defines task rollout and the shared observation vector.
- `qd.rs` implements MAP-Elites archive insertion, replacement, and sampling.
- `evolution.rs` ties evaluation, archive maintenance, and parent selection together.

## Current Scope

The Rust core is complete and covered by regression tests, and a browser surface now sits on top of it: a 2D creature simulator at `/`, a 3D evolved-stride viewer at `/3d` (orbit camera, follow mode, terrain presets; three.js from CDN), and a `/api/replay` endpoint that serves both minimal random genomes and `mode=evolved` replays evolved on demand. The next natural layer is archive grid browsing, lineage history, and persistent checkpoints in the UI.

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
