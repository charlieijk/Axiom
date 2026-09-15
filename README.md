# Axiom

> A 3D game where you build and evolve your own robots.

Design a quadruped, evolve how it moves, and put it through physical trials.
Axiom combines an interactive 3D game with a robotics workshop: build, evolve,
test, inspect, and try again. Rust and Rapier simulate the robot's body, joints,
servos and ground contacts; the browser shows the actual simulated motion.

In the current 3D game, **you customize the robot's body and evolution optimizes
its movement controller**. Body dimensions do not evolve automatically in this
mode. The earlier planar body-and-neural-controller experiments remain available
as separate research tools.

## Play

From this checkout, with Rust 1.89 or newer installed:

```sh
bin/axiom play
```

Open **http://127.0.0.1:8790** in a browser with WebGL support. The first launch
builds the game. If that port is already held (a second `bin/axiom play`, a stale
server), the game takes the next free port within twenty and prints the address
it actually bound; `bin/axiom play --port 9000` picks one. The equivalent
command is:

```sh
cargo run --release --locked -p axiom-field -- lab [--port N]
```

Start with the reference trot or choose **Evolve → Load bundled archive** to
explore 43 controllers from the earlier 576-candidate search. The bundled archive
restores its original robot and loads its highest-scoring training controller;
new trials measure how that controller performs in the selected course.

**Controls:** drag to orbit, scroll to zoom, **Space** to start/pause, and **R**
to reset. Keyboard shortcuts leave focused form controls alone.

## Build, evolve, and test

- **Trial:** walk one metre upright on flat ground or three physical 6 mm rails.
  Tune the manual trot, pause, reset, and inspect foot contacts with the orbit camera.
- **Build:** adjust chassis size/mass, leg lengths and friction. Applying a design
  rebuilds the real collision geometry and clears scores from the previous design.
- **Evolve:** load the bundled 43-controller repertoire from the earlier
  576-candidate search with its nominal robot, or run seeded MAP-Elites across three perturbed flat-ground worlds.
  Inspect speed/economy cells, select a complete eight-joint CPG controller, then
  test it on either course. Optional holdout tests re-score the archive against
  three unseen parameter worlds. A background worker reports progress and supports
  cancellation; incomplete searches do not replace the previous archive.
- **Replays:** save and scrub actual Rust collider poses. Completed trials and
  trials reset after movement are recorded automatically; the latest three are
  retained in memory. Viewing a recording never advances the live simulation.
- **Save / import:** download a versioned workshop JSON with design, active gait,
  full selected controller and archive. Import validates the whole save before
  replacing state; imported metrics are labelled as imported. Download recordings
  separately to keep them beyond the running session.

Rust owns physics, servo limits, fixed ticks, falling, completion and the
60-second trial limit. The browser renders authoritative collider transforms.
Pausing stops simulation time. The robot and holdout results remain uncalibrated
against hardware. Search uses 12 candidates per generation, 120 ticks per world,
three worlds, seed 0–4294967295 and 1–12 generations. A short search does not
promise a faster controller or a full archive.

The server is a single-user local tool bound to `127.0.0.1:8790` (or the port
it printed), with same-origin JSON commands keyed to that bound port and a
256 KiB import limit. State is in memory: export a workshop
before stopping the process. Holdout results are recomputed, not trusted from an
import. The existing planar morphology/neural-controller research, checkpoint
handoffs and `/3d` journal remain available through `bin/axiom gui` and the root
CLI. Their abstract planar physics is a distinct model, so planar genomes are
not silently substituted for physical quadruped controllers.

## Development

```sh
cargo test --workspace --locked
npm ci
npm test
npm run lint
cargo clippy --workspace --all-targets -- -D warnings
```

The playable game lives in `crates/axiom-field/`: `sim.rs` owns rigid-body physics,
`controller.rs` defines the eight-joint CPG, `search.rs` runs MAP-Elites,
`workshop.rs` manages designs and archives, and `lab.rs` serves the local game and
records trials. Its browser interface is in `crates/axiom-field/web/`.
The separate `site/` directory is the product website, not the simulation engine.
See [AGENTS.md](AGENTS.md) for the complete development and verification commands.

## Research tools and benchmark history

The original planar experiments, checkpoint workflows, and Arm64 benchmarks are
preserved below. Their measurements describe that research engine; they are not
performance claims for the rigid-body 3D game. [DEMO.md](DEMO.md) walks through
the earlier research workflow.

<details>
<summary>Planar research tools, reproducible experiments, and benchmarks</summary>

## Arm64 optimization

This work was built for the **Arm Create: AI Optimization Challenge**
(Track 1 — Physical AI), submitted August 2026. That challenge has since
closed; the optimization, the measurements, and the reproduction protocol
below are kept because they stand on their own.

The track fit the project as it already was: simulated sensor observations
drive neural-controller inference, actuator commands, embodied motion, fitness,
and MAP-Elites selection in one deterministic loop.

Axiom's deterministic rollout hot path now reuses observation, neural-network,
action, and snapshot buffers. On an Apple M2 Arm64 host, the full fixed-seed
workload used **94.38% fewer allocation/reallocation calls** and requested
**89.73% fewer allocation bytes**, while the complete evolution report remained
identical to the pre-optimization baseline. See the reproducible protocol, raw
JSON, source hashes, and limitations in
[docs/ARM64_OPTIMIZATION.md](docs/ARM64_OPTIMIZATION.md). The screenshot sequence and
recording plan prepared for the submission are in
[docs/SUBMISSION_ASSETS.md](docs/SUBMISSION_ASSETS.md).

Anyone on Arm64 can reproduce this with the copy-paste quick verification and
check the result against its expected semantic digest — written for the
challenge's judges, and still the fastest way to confirm the claim:
[Arm64 quick check](docs/ARM64_OPTIMIZATION.md#arm64-judge-quick-check).
Apple Silicon release usage and the source-build fallback are documented in
[docs/APPLE_SILICON.md](docs/APPLE_SILICON.md).

![Axiom's measured Apple M2 Arm64 allocation result](docs/axiom-arm64-result.png)

![Axiom Archive Lab showing a live evolved creature and selectable MAP-Elites repertoire](docs/axiom-archive-lab-gen12.jpg)

The planar research engine includes:

- morphology genomes with directional attachments
- compiled neural controllers with explicit bias handling
- feedforward, recurrent, and CPG controller modes
- stateful rollout semantics shared by evaluation and playback-style code
- deterministic simulation with replace-on-respawn behavior
- task fitness for flat and rough terrain
- MAP-Elites quality-diversity search
- regression tests for the correctness failures called out in review

## Research CLI

The [`bin/axiom`](bin/axiom) CLI wraps every command this repo runs. Put it on
your PATH once with `bin/axiom link` (symlinks into `~/.local/bin`), then:

```sh
axiom test
axiom demo
axiom gui
axiom animate cpg
axiom evaluate cpg
axiom evolve --generations 10 --population 32 --steps 180 --save checkpoints/run.json
axiom evolve --pack rough-inspection --report-json reports/inspection.json --report-md reports/inspection.md
axiom inspect checkpoints/run.json
axiom animate --checkpoint checkpoints/run.json --frames 160 --fps 20
axiom handoff checkpoints/run.json --output exports/candidate-shortlist.json
axiom bench
axiom bench arm64
axiom verify
```

`axiom help` lists everything, including `lint`, `coverage`, the site tasks,
and the allocation-counting benchmark variant. Simulation commands forward to
the Rust binary unchanged (`axiom gui` ≡ `cargo run --locked -- gui`; add `-r`
for a release build), and the development commands run the same invocations as
CI — the canonical `cargo` forms remain in [AGENTS.md](AGENTS.md). Installing
the crate itself (`cargo install --path .`) also yields an `axiom` binary with
the simulation commands.

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
- `handoff.rs` turns archive diversity into a versioned, downstream-importable creature shortlist.

## Planar research interface

The Rust core is complete and covered by regression tests, and a browser surface now sits on top of it: a 2D creature simulator at `/`, a 3D evolved-stride viewer at `/3d` (orbit camera, follow mode, terrain presets, and a selectable MAP-Elites Archive Lab; Three.js r165 is bundled for offline use), and a `/api/replay` endpoint that serves both minimal random genomes and `mode=evolved` replays evolved on demand. Evolved replays now carry the selected genome's real parent lineage and mutation record rather than inferring ancestry from generation aggregates.

## Reproducible Experiments

Save the full configuration, champion, archive, generation history, and lineage in a versioned checkpoint:

```sh
cargo run -- evolve --generations 16 --population 32 --steps 180 --seed 42 --save checkpoints/run-42.json
cargo run -- inspect checkpoints/run-42.json
cargo run -- animate --checkpoint checkpoints/run-42.json --frames 160 --fps 20
```

For a deterministic inspection across rough terrain, tilt recovery, and a step
field, pass `--pack rough-inspection`. `--report-json` and `--report-md` write
human- and machine-readable summaries without replacing the complete,
versioned checkpoint.

Checkpoint writes use a temporary sibling file and atomic replacement, so an interrupted save does not leave a partially written destination. Unknown checkpoint versions and malformed files fail with an explicit load error. Archive dimensions must be non-zero and match the serialized cell count exactly, so a truncated or internally inconsistent repertoire is refused before inspection, replay, or handoff.

Move from an experiment to downstream creature design with a deterministic shortlist:

```sh
cargo run -- handoff checkpoints/run-42.json --output exports/run-42-shortlist.json
```

The `axiom.candidate-shortlist.v1` document preserves each selected genome and evaluation while
assigning practical roles: champion, stable, explorer, and lean. One genome may fill several
roles. The handoff carries the checkpoint seed, archive context, selection policy, and explicit
software-only / no-hardware-validation limitations; it does not claim engine-specific import or
sim-to-real readiness.

To compare many runs at once — archive coverage, QD-score, and best fitness across seeds and generation budgets — export the checkpoints to Parquet and query them with DuckDB. The checkpoint format is unchanged; the export is a read-only columnar copy:

```sh
pip install -r requirements-duckdb.txt
python script/export_checkpoints_parquet.py
```

See [docs/duckdb.md](docs/duckdb.md) for the schema and a worked cross-seed comparison.

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

</details>

## License

MIT — see [LICENSE](LICENSE).
