# Flagship Demo

## Promise

Axiom makes embodied evolution reproducible and inspectable: morphology,
controller, terrain, archive position, and replay stay connected.

## Judge demo (2 minutes 45 seconds)

1. Lead with [the Arm64 result card](docs/axiom-arm64-result.png): the optimized
   rollout uses 94.38% fewer allocation calls and requests 89.73% fewer bytes,
   with the complete evolution report unchanged.
2. Run `cargo run --locked -- gui`, open `http://127.0.0.1:8787/3d`, and identify
   the live morphology, CPG controller, rough-terrain task, fitness, archive
   coverage, and lineage.
3. Select Archive Lab cell 0,7 and contrast its 9-part, 0.86-stability elite
   with the champion in cell 6,5. Then compare generation 6 with generation 12.
4. Choose **Evolve next generation**. Show the replay, progress chart, Archive
   Lab, journal, and timeline updating from the Rust backend together.
5. Use [the core-loop map](docs/axiom-core-loop.png) to trace observation
   → brain → action → simulation → fitness → MAP-Elites.
6. Open [docs/ARM64_OPTIMIZATION.md](docs/ARM64_OPTIMIZATION.md), connect the
   reusable buffers to the two measured allocation results, and show the full
   report SHA-256 correctness gate. State that runtime speedup is not claimed.
7. Close on the moving creature: allocator traffic fell without changing the
   experiment's result.

The exact shot sequence, captions, timed narration, and recording checklist are
in [docs/SUBMISSION_ASSETS.md](docs/SUBMISSION_ASSETS.md).

## Proof gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
node --test web/tests/*.test.mjs
cargo run --release --locked --example arm64_optimization_benchmark -- --quick
cargo run --locked -- gui --check
cargo package --locked
```
