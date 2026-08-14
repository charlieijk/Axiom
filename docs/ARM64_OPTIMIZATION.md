# Arm64 rollout optimization

Axiom's challenge entry optimizes the allocation-heavy inference and simulation hot path used to evaluate embodied agents. On an Apple M2 Arm64 host, the optimized build removed **94.38% of allocation/reallocation calls** and **89.73% of requested allocation bytes** across a deterministic 2,048,400-step MAP-Elites workload. The complete 1,710,519-byte evolution report remained identical to the pre-optimization baseline.

## Challenge track and update scope

Axiom targets **Track 1 — Physical AI**. It closes the loop between simulated
sensor observations, neural-controller inference, actuator commands, embodied
motion, task fitness, and MAP-Elites selection. The challenge work began from
public baseline commit `71d3897dc04f90a4903a14fd737af1ad680cef7c`
(August 10, 2026). The significant post-start update is the reusable workspace
path described below, plus the isolated baseline/optimized Arm64 harness and its
semantic-equivalence gates. The search algorithm, workload, seed, and scalar
operation order did not change to manufacture the result.

## What changed

The baseline allocated fresh vectors while producing observations, running the neural controller, collecting actions, and taking simulation snapshots at every rollout step. The optimized path creates scratch storage once per evaluation and reuses it:

- `CompiledNetwork::forward_into` reuses neural value and output buffers.
- `Brain::think_into` reuses controller input and network workspaces.
- `observation_vector_into` reuses the sensor vector.
- `Simulation::snapshot_into` reuses joint-state storage with `clone_from`.

The existing allocating APIs remain as compatibility wrappers. The rollout preserves scalar operation order; it does not add approximate math or change the search algorithm.

## Measured result

Both binaries used the same benchmark source, lockfile, hardware, toolchain, workload, and allocation-counting implementation. The baseline core came from commit `71d3897dc04f90a4903a14fd737af1ad680cef7c`; the optimized core is the buffer-reuse working tree based on that commit.

| Metric | Baseline | Optimized | Change |
| --- | ---: | ---: | ---: |
| Allocation/reallocation calls | 13,000,529 | 730,613 | **94.38% fewer** |
| Requested allocation bytes | 2,146,339,344 | 220,528,660 | **89.73% fewer** |
| Calls per simulation | 2,538.67 | 142.67 | 2,396.00 fewer |
| Requested bytes per rollout step | 1,047.81 | 107.66 | 940.15 fewer |

“Requested bytes” is allocator request volume, including reallocations. It is not peak RSS or resident memory.

## Correctness contract

The fixed-seed benchmark compares the entire `EvolutionReport` between repeated runs and gates the full workload against its pre-optimization digest. That report includes the champion, all evaluation metrics, all archive elites, 40 generation summaries, and 3,872 lineage records.

| Observable | Baseline and optimized |
| --- | ---: |
| Full report digest | `be0a71bc9f057db4` |
| Full report SHA-256 | `a8118314dbb8960844a2b58787e5a9d892e4f9556e503db978c60a23b3a321a4` |
| Serialized report size | 1,710,519 bytes |
| Best-fitness bits | `1141730189` |
| Stable-distance bits | `1108109660` |
| Occupied archive cells | 60 |
| Reported evaluated genomes | 5,120 |

The reusable buffer APIs also have focused wrapper-versus-workspace equality tests, including multi-step traces for feedforward, recurrent, and CPG controllers.

## Workload and host

- Apple M2 MacBook Air (`Mac14,2`), 8 physical/logical cores, 8 GiB memory
- Arm64 macOS 26.5.2; NEON target feature present
- Rust/Cargo 1.96.1, LLVM 22.1.2, release profile, no custom `RUSTFLAGS`
- seed 1337; mixed feedforward, recurrent, and CPG controllers
- population 128; 40 generations; 400 evaluation steps
- rough-terrain task; MAP-Elites archive 12 × 8
- 5,121 simulation calls and 2,048,400 rollout steps on one thread

The production optimization is portable buffer reuse measured and validated on Arm64. It does **not** claim a hand-written NEON path. The Field Lab's deterministic Rapier configuration is separate and is not described as SIMD-accelerated.

## Reproduce

Run uninstrumented timing and allocation counting as separate release binaries:

```sh
cargo run --release --locked --example arm64_optimization_benchmark -- --full
cargo run --release --locked --features allocation-counting \
  --example arm64_optimization_benchmark -- --full --allocations
```

The benchmark refuses authoritative execution on non-Arm64 hosts unless `--allow-non-arm64` is explicitly supplied. Allocation-counting builds refuse timing mode because even disabled allocator instrumentation perturbs an allocation-heavy hot path.

The pre-optimization binary was rebuilt in an isolated `git archive` of commit `71d3897d`; only the current benchmark example and empty `allocation-counting` feature declaration were added to that tree. Source and binary hashes are recorded in the raw artifacts.

## Arm64 judge quick check

Prerequisites: an Arm64 macOS or Linux host, Git, and Rust 1.85 or newer. The
authoritative measurements used Rust 1.96.1; `Cargo.lock` pins dependencies.

```sh
git clone https://github.com/charlieijk/Axiom.git
cd Axiom
uname -m
cargo test --locked --test arm64_evidence
cargo run --release --locked --example arm64_optimization_benchmark -- --quick
```

Expected checks:

- `uname -m` prints `arm64` on macOS or `aarch64` on Linux.
- `checked_in_arm64_comparison_matches_raw_artifacts ... ok` confirms that the
  checked-in baseline, optimized, and comparison JSON agree with the current
  benchmark and optimized source hashes.
- The quick benchmark emits JSON with `"authoritative_arm64": true`,
  `"arch": "aarch64"`, and `"neon": true`.
- Its fixed-seed quick report SHA-256 is
  `e2600ca19373603be085a1e33ada40e6128c20ec76408fe5b61de890b46dbff7`.

The quick command is a smoke test, not the source of the headline reductions.
Those come from the full allocation-counting runs recorded below. To reproduce
the optimized full measurement (several minutes depending on the host), run:

```sh
cargo run --release --locked --features allocation-counting \
  --example arm64_optimization_benchmark -- --full --allocations
```

## Evidence

- [Baseline allocation report](benchmarks/arm64-allocation-baseline.json)
- [Optimized allocation report](benchmarks/arm64-allocation-optimized.json)
- [Machine-readable comparison](benchmarks/comparison.json)

## Timing limitation

Exploratory paired timings indicated a favorable direction but were too variable for a defensible speedup claim on the development machine. Runtime is therefore not used as the submission headline. The benchmark's timing mode is now uninstrumented and records raw samples, so a controlled host can add a runtime result later without changing the measurement contract.
