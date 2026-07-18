# Flagship Demo

## Promise

Axiom makes embodied evolution reproducible and inspectable: morphology,
controller, terrain, archive position, and replay stay connected.

## Five-minute flow

1. Run `cargo run -- gui` and open `http://127.0.0.1:8787`.
2. Start from the field journal and identify the task, controller, and archive.
3. Run a short evolution and compare the emerging elites instead of showing
   only one winner.
4. Replay a selected body on rough terrain and connect its motion back to the
   recorded experiment.

## Proof gate

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```
