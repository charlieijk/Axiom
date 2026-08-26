# Title

Axiom: Allocation-Efficient Embodied Evolution on Arm64

## One-line Summary

A deterministic Physical AI sandbox whose Arm64 rollout uses 94.38% fewer allocation calls and requests 89.73% fewer allocation bytes while preserving the complete evolution result.

## Problem

Embodied-evolution systems evaluate thousands of candidate bodies and controllers across long simulation rollouts. In Axiom's original hot path, sensor observations, neural-network state, actuator outputs, and simulation snapshots repeatedly allocated new vectors. That allocator traffic was unnecessary, but removing it could not be allowed to change controller state, floating-point operation order, fitness, archive contents, or lineage.

Physical AI also needs more than a final score. Developers and reviewers need to inspect how sensor input becomes control, how a body behaves on terrain, why a candidate occupies a quality-diversity cell, and whether an optimization preserved the experiment it was meant to improve.

## Solution

Axiom is a Rust embodied-evolution sandbox for **Track 1 — Physical AI**. It closes the loop from simulated sensor observations through feedforward, recurrent, or central-pattern-generator neural-controller inference, actuator commands, embodied motion, task fitness, and MAP-Elites selection.

For this challenge, Axiom replaced repeated hot-path allocations with reusable observation, neural-network, action, and snapshot workspaces. The existing allocating APIs remain as compatibility wrappers. The search algorithm, workload, seed, and scalar operation order were left unchanged.

On an Apple M2 Arm64 host, the fixed 2,048,400-step workload changed from 13,000,529 to 730,613 allocation/reallocation calls—**94.38% fewer**—and from 2,146,339,344 to 220,528,660 requested allocation bytes—**89.73% fewer**. The complete 1,710,519-byte evolution report retained the same SHA-256 before and after the optimization, including its champion, 60 occupied archive cells, 40 history entries, and 3,872 lineage records.

### Significant challenge-period update

Axiom existed before the challenge. Starting from baseline commit [`71d3897d`](https://github.com/charlieijk/Axiom/commit/71d3897dc04f90a4903a14fd737af1ad680cef7c) on August 10, 2026, the challenge work added reusable rollout workspaces, an isolated Arm64 allocation harness, full-report equivalence gates, native Arm64 CI and release automation, judge documentation and assets, and presentation refinements. The baseline and optimized measurements use the same hardware, toolchain, lockfile, benchmark source, workload, and fixed seed.

## Why This Matters

Axiom combines a material Arm64 optimization with a strict correctness contract and a working judge-facing experience:

- **Technological implementation (40 points):** raw baseline and optimized JSON record benchmark, lockfile, optimization-source, binary, hardware, toolchain, and outcome hashes. Focused tests compare allocating and workspace APIs across feedforward, recurrent, and CPG controllers.
- **User/developer experience (15 points):** judges can inspect a live 3D creature, select different MAP-Elites cells, inspect real lineage, advance evolution, verify the checked-in comparison, and reproduce the optimized measurement with copy-paste commands.
- **Potential impact (20 points):** reusable workspaces are a portable optimization pattern for allocation-heavy inference and simulation loops on Arm64. The checked-in evidence shows reduced allocator request volume without approximate math or changed search semantics.
- **WOW factor (25 points):** the optimization lives inside a complete, visible Physical AI loop rather than a benchmark alone—the moving creature, archive diversity, history, lineage, and evidence stay connected.

## How We Used AI

Axiom's AI is neuroevolution rather than a prompt wrapper. Each genome describes a body morphology and a neural controller. During every rollout, simulated observations feed a feedforward, recurrent, or CPG controller; its outputs become actuator commands; and the resulting motion is scored for progress and stability on flat, rough, or recovery terrain.

MAP-Elites preserves the strongest candidate in each behavior cell instead of collapsing the search to one global winner. This produces an inspectable repertoire of distinct morphologies and behaviors, with parent lineage and mutation records attached to evolved genomes.

The challenge optimization targets this AI evaluation loop directly. `observation_vector_into`, `Brain::think_into`, `CompiledNetwork::forward_into`, and `Simulation::snapshot_into` reuse storage across rollout steps while preserving scalar operation order and the complete experiment result.

## How We Used Codex

Codex served as an engineering copilot throughout the challenge update. It helped profile the allocation-heavy rollout path, implement reusable observation/network/action/snapshot workspaces, construct an isolated baseline-versus-optimized Arm64 evidence harness, and strengthen semantic-equivalence tests.

Codex also helped connect implementation claims to reproducible raw JSON, source hashes, CI gates, architecture documentation, judge screenshots, and the demo flow. It assisted with security and release-readiness review, including secret scanning, dependency checks, build/test verification, and identifying remaining supply-chain and local-service hardening work.

## Key Features

- Co-evolution of morphology and feedforward, recurrent, or CPG neural controllers.
- Deterministic evaluation on flat, rough-terrain, and recovery tasks.
- MAP-Elites quality-diversity archive with selectable elites.
- Real parent lineage and mutation records for evolved replays.
- Rust-backed 2D simulator and interactive 3D Archive Lab.
- Orbit camera, follow mode, terrain presets, progress history, and next-generation interaction.
- Versioned checkpoints containing configuration, champion, archive, history, and lineage.
- Reproducible Arm64 evidence with raw baseline/optimized JSON, hashes, asset checksums, and semantic-equivalence tests.
- MIT-licensed source with a vendored Three.js license.

## Architecture

The Rust core is authoritative for genomes, controller execution, deterministic simulation, task fitness, evolution, MAP-Elites, checkpoints, lineage, and replay JSON.

```text
observations
    → neural controller
    → actuator commands
    → deterministic simulation
    → fitness
    → MAP-Elites archive and parent selection
    → next candidate genome
```

The reusable workspaces sit inside the observation → controller → action → simulation hot path. A loopback-only Rust server exposes validated replay data through `/api/replay`. The embedded 2D and 3D clients render that data but do not independently score or evolve genomes. The separate presentation site is not a second source of simulation truth and is not being offered as a public judge demo.

Judge-readable diagrams:

- [`docs/axiom-core-loop.png`](docs/axiom-core-loop.png) — large-type Physical AI loop and optimization placement.
- [`docs/axiom-architecture.png`](docs/axiom-architecture.png) — full authority and trust-boundary map.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — source-backed architecture notes and limitations.

## Testing Instructions

### Arm64 judge quick check

Prerequisites: Git, Rust 1.85 or newer, and an Arm64 macOS or Linux host. The authoritative measurements used Rust/Cargo 1.96.1 on an Apple M2; `Cargo.lock` pins dependencies.

```sh
git clone https://github.com/charlieijk/Axiom.git
cd Axiom
uname -m
cargo test --locked --test arm64_evidence
cargo run --release --locked --example arm64_optimization_benchmark -- --quick
cargo run --locked -- gui --check
cargo run --release --locked --bin axiom -- gui
```

Open the printed local address with `/3d` appended, normally `http://127.0.0.1:8787/3d`.

Expected evidence:

- `uname -m` reports `arm64` on macOS or `aarch64` on Linux.
- The evidence test confirms that the checked-in baseline, optimized, and comparison JSON agree with the benchmark and optimized source hashes.
- The quick benchmark emits `"authoritative_arm64": true`, `"arch": "aarch64"`, and the fixed-seed report SHA-256 `e2600ca19373603be085a1e33ada40e6128c20ec76408fe5b61de890b46dbff7`.

### Full optimized allocation measurement

```sh
cargo run --release --locked --features allocation-counting \
  --example arm64_optimization_benchmark -- --full --allocations
```

The full recorded baseline, optimized, and comparison artifacts are under [`docs/benchmarks/`](docs/benchmarks/). The headline result comes from these full allocation-counting runs, not the quick smoke test.

### Full repository verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
node --test web/tests/*.test.mjs
npm --prefix site test
npm --prefix site run lint
cargo run --locked -- gui --check
shasum -a 256 -c docs/submission-assets.sha256
```

## Public Demo Link

**TODO — no public live Rust demo is currently published.**

Local judge fallback after cloning:

```sh
cargo run --release --locked --bin axiom -- gui
```

Then open `http://127.0.0.1:8787/3d`. The local service is intentionally unauthenticated and loopback-only; it must not be exposed directly to a network.

## Public Repository Link

https://github.com/charlieijk/Axiom

The repository is public, its MIT license is detected by GitHub, and `main` contains the complete source, assets, evidence, and judge instructions required to build and test Axiom.

Immutable reviewer links for the challenge implementation snapshot:

- Source: `https://github.com/charlieijk/Axiom/tree/6a1986b6b2bc6841052c7c14d0d0873f0b899e76`
- Evidence protocol: `https://github.com/charlieijk/Axiom/blob/6a1986b6b2bc6841052c7c14d0d0873f0b899e76/docs/ARM64_OPTIMIZATION.md#arm64-judge-quick-check`

## Demo Video

**Optional; not yet recorded or published.** Final URL: `TODO — public YouTube, Vimeo, or Youku URL`.

Target length: **2 minutes 45 seconds**.

| Time | Visual | Narration target |
| --- | --- | --- |
| 0:00–0:15 | `docs/axiom-arm64-result.png` | Introduce Axiom and state the two measured allocation reductions. |
| 0:15–0:40 | Live `/3d` Archive Lab on rough terrain | Identify morphology, CPG controller, fitness, archive coverage, and lineage; note that the interface reflects Rust replay data. |
| 0:40–1:03 | Select Archive Lab cell 0,7 | Contrast its 9-part, 0.86-stability elite with the champion in cell 6,5 and explain MAP-Elites. |
| 1:03–1:18 | Compare generations 6 and 12 | Show verified fitness/coverage changing from 87.98/10% to 121.46/15%. |
| 1:18–1:35 | Choose **Evolve next generation** | Show replay, chart, archive, journal, and timeline advancing; state honestly that the champion remains unchanged. |
| 1:35–1:52 | `docs/axiom-core-loop.png` | Trace observations → brain → action → simulation → fitness → MAP-Elites. |
| 1:52–2:27 | Optimization document and result table | Explain reusable workspaces and the 94.38%/89.73% reductions over 2,048,400 steps. |
| 2:27–2:40 | Correctness table | Show the identical full-report SHA-256 and the preserved champion, archive, history, and lineage. |
| 2:40–2:45 | Return to the moving creature | Close: Axiom creates far less allocator traffic without changing the experiment's result. |

Do not use copyrighted music, third-party marks without permission, or footage that implies unsupported hardware validation.

## Screenshot Shot List

Use [`docs/axiom-arm64-result.png`](docs/axiom-arm64-result.png) as the cover/thumbnail, then upload the product and architecture images in this order.

1. [`docs/axiom-arm64-result.png`](docs/axiom-arm64-result.png)  
   **Caption:** On Apple M2 Arm64, Axiom's reusable rollout workspaces cut allocation/reallocation calls by 94.38% and requested allocation bytes by 89.73% across 2,048,400 deterministic steps, while the complete evolution report stayed identical.
2. [`docs/axiom-archive-lab-gen12.jpg`](docs/axiom-archive-lab-gen12.jpg)  
   **Caption:** A live CPG-controlled champion traverses rough terrain while the Field Journal connects its 7-part morphology, fitness 121.46, 15% archive coverage, recorded lineage, and selected MAP-Elites cell 6,5.
3. [`docs/axiom-archive-elite-cell-0-7.jpg`](docs/axiom-archive-elite-cell-0-7.jpg)  
   **Caption:** Archive Lab cell 0,7 replays Genome 210, a 9-part, 0.86-stability elite from generation 9. Its distinct morphology shows why Axiom preserves a repertoire instead of only one winner.
4. [`docs/axiom-core-loop.png`](docs/axiom-core-loop.png)  
   **Caption:** Sensor observations feed neural-controller inference and actuator commands; simulation metrics return to fitness and MAP-Elites selection. Reusable Arm64-measured workspaces optimize that loop without changing scalar operation order.
5. [`docs/axiom-archive-lab-gen13.jpg`](docs/axiom-archive-lab-gen13.jpg)  
   **Caption:** Evolve next generation advances the run to generation 13 and refreshes replay, progress chart, Archive Lab, journal, and timeline. The champion honestly remains unchanged in this deterministic step.

Use [`docs/axiom-architecture.png`](docs/axiom-architecture.png) as a sixth image only if another gallery slot is available. The six raster assets are covered by [`docs/submission-assets.sha256`](docs/submission-assets.sha256).

## Submission Readiness Notes

### Verified locally

- Arm challenge registration is live and verified for the authenticated Devpost account.
- The current challenge deadline is August 14, 2026 at 4:00 PM Pacific.
- Rust formatting, clippy, workspace tests, release build, GUI smoke check, web tests, Flask tests, site build/lint/tests, asset checksums, and 84.63% line coverage passed locally.
- `npm audit --omit=dev --audit-level=high` reported zero vulnerabilities.
- `gitleaks git --no-banner --redact` found no secrets in Git history.
- `cargo package --locked` verified the clean package contents (61 files, 2.0 MiB uncompressed).
- A fresh full optimized Arm64 allocation run exactly reproduced 730,613 allocation/reallocation calls, 220,528,660 requested bytes, and the checked-in full-report SHA-256.
- Live GitHub CI is green, including its native Arm64 evidence lane.
- The sealed security review reported one medium release-supply-chain issue and four low local/demo hardening issues; no secrets were found.

### Resolved requirements and optional follow-ups

- The repository is public, MIT-licensed, and anonymously accessible, satisfying the official repository requirement.
- Judges build the current public `main` branch; the immutable implementation links above preserve the measured challenge snapshot.
- Pin or otherwise remediate the medium release-workflow supply-chain finding before triggering the automated release job.
- Decide whether to record the optional video; if yes, publish it publicly and insert the URL.
- Optionally provide a public demo URL; the event does not require one.

### Built with

Suggested Devpost tags: `Rust`, `Arm64`, `Apple Silicon`, `Neuroevolution`, `MAP-Elites`, `Three.js`, `Serde`.

## Known Limitations

- The evidence measures allocator calls and allocator-requested bytes; requested bytes are not peak RSS, resident memory, or total memory saved.
- Exploratory timing was too variable for a defensible runtime-speedup claim, so no speedup is claimed.
- No energy reduction is claimed.
- The production optimization is portable buffer reuse measured on Arm64, not handwritten NEON/SIMD code.
- The demonstrated Physical AI system is simulated. Field Lab robot parameters are uncalibrated, and no physical-robot or sim-to-real validation is claimed.
- The separate presentation site does not run the authoritative Rust evolution backend and is not offered as a public judge demo.
- The local Rust GUI is an unauthenticated single-user loopback tool. Non-loopback use requires an authenticated TLS boundary and resource controls.
- No public challenge binary release, public live demo, or finished video exists; none is required by the live form, so judges use the source-build path.

## Official Form Fields — Completed Draft Answers

The live form does **not** request a Codex session ID. Do not add one.

### Required field 27623

**What was the hardest part of building or optimizing your project? Select all that apply**

Answer: `Measuring performance`.

### Required field 27624

**What would have made it easier to complete your project? Select all that apply.**

Answers: `More benchmarking examples`; `More Arm-specific optimization guidance`.

### Required field 27625

**Did this challenge change your likelihood of building on Arm in the future?**

Answer: `Yes, somewhat more likely`.

### Required field 27626

**How likely are you to continue developing, optimizing, or deploying this project after the challenge?**

Answer: `Very likely`.

### Optional field 27628

**What is one thing Arm could improve to better support developers like you?**

Answer:

> Provide standardized native Arm benchmark runners and reproducibility guidance so developers can make defensible baseline-versus-optimized comparisons across devices.

### Final links and media

- Repository URL: `https://github.com/charlieijk/Axiom` — public, MIT-licensed, and ready for anonymous judging access.
- Public demo URL: omitted (optional; no public demo exists).
- Video URL: omitted (optional; no video exists).
- Cover/thumbnail: `docs/axiom-arm64-result.png`.
- Target track: `Track 1 — Physical AI`.
