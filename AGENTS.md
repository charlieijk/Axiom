# Axiom

> Axiom is a Rust embodied-evolution sandbox.

## What this is

Axiom is a Rust embodied-evolution sandbox. It evolves simple body plans and neural controllers across terrain tasks, then keeps diverse elites in a MAP-Elites archive instead of collapsing everything to one winner.

## Stack

- Rust (edition 2024)
- React 19.2.6
- Vite 8.0.13
- TypeScript 5.9.3
- Python

## Quick start

```bash
cargo run
```

## Commands

| Task | Command |
| --- | --- |
| Run | `cargo run` |
| Test | `cargo test` |
| Lint | `cargo clippy --all-targets` |
| Build | `cargo build --release` |
| Test (site/) | `npm --prefix site test` |
| Lint (site/) | `npm --prefix site run lint` |
| Build (site/) | `npm --prefix site run build` |

## Layout

| Path | Purpose |
| --- | --- |
| `checkpoints/` |  |
| `crates/` |  |
| `design/` |  |
| `docs/` | Long-form documentation |
| `examples/` | Runnable examples |
| `release/` |  |
| `site/` | Marketing/product website |
| `src/` | Application source |
| `tests/` | Test suite |
| `web/` | Browser client |
| `.github/` | CI workflows and Dependabot configuration |

## Conventions

- **Commits:** Conventional Commits (`feat:`, `fix:`, `docs:`, `ci:`, `refactor:`).
- **Branching:** work on a branch and open a PR; do not self-merge.
- **Licence:** MIT.
- **CI:** `ci.yml`, `gitleaks.yml` must be green.

## Before you finish

- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets` passes
