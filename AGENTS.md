# Axiom

> A Rust embodied-evolution sandbox with inspectable quality-diversity search and deterministic replay.

## What this is

Axiom evolves body plans and neural controllers across terrain tasks, preserving diverse elites in a MAP-Elites archive rather than selecting only one winner. The root crate owns the deterministic simulation, evaluation, evolution, checkpoints, CLI, and replay server. `crates/axiom-field/` adds a rigid-body quadruped lab for robust-gait experiments, while `web/` and `site/` provide distinct browser surfaces over the project.

## Stack

- Rust edition 2024; MSRV 1.85 for the publishable `axiom` crate and 1.89 for `axiom-field`
- Serde 1 and serde_json 1 for persisted experiment and replay data
- Rapier 3D 0.34 and TOML 1.1 in the non-publishable Field Lab crate
- Browser-native JavaScript with Three.js r165 for the embedded replay viewer
- Node.js 22.13.0 or newer, React 19.2.6, TypeScript 5.9.3, Vite 8.0.13, and Vinext 1.0.0-beta.2 for `site/`
- Python 3.10 or newer with Flask 3.1.3 and Requests 2.32.5 for the optional loopback compatibility adapter

## Quick start

```sh
cargo run --locked -- gui
```

Open the loopback URL printed by the command. The default is `http://127.0.0.1:8787`.

## Commands

`bin/axiom` wraps these as short subcommands (`axiom test`, `axiom lint`,
`axiom bench arm64`, `axiom verify`) and forwards simulation commands to the
Rust binary; the table below stays the canonical, CI-verified form.

| Task | Command |
| --- | --- |
| Install site dependencies | `npm --prefix site ci` |
| Run the Rust GUI | `cargo run --locked -- gui` |
| Run the public site | `npm --prefix site run dev` |
| Test the Rust workspace | `cargo test --workspace --locked` |
| Measure Rust coverage | `cargo llvm-cov --workspace --fail-under-lines 80` |
| Test embedded browser logic | `node --test web/tests/*.test.mjs` |
| Test the public site | `npm --prefix site test` |
| Test the Flask adapter | `python -m unittest discover -s tests_python -p 'test_*.py' -v` |
| Check Rust formatting | `cargo fmt --all -- --check` |
| Lint the Rust workspace | `cargo clippy --workspace --all-targets -- -D warnings` |
| Lint the public site | `npm --prefix site run lint` |
| Build the Rust release | `cargo build --workspace --release --locked` |
| Build the public site | `npm --prefix site run build` |
| Smoke-test bundled GUI assets | `cargo run --locked -- gui --check` |
| Verify the publishable package | `cargo package --locked` |
| Audit Rust dependencies | `cargo audit` |
| Scan Git history for secrets | `gitleaks git --no-banner --redact` |

## Layout

| Path | Purpose |
| --- | --- |
| `src/` | Publishable Rust library, CLI, deterministic sandbox, evolution engine, checkpoints, and replay server |
| `crates/axiom-field/` | Non-publishable rigid-body quadruped simulator and robust-gait search lab |
| `tests/` | Root-crate integration tests, including the checkpoint CLI journey |
| `tests_python/` | Flask adapter trust-boundary and proxy tests |
| `examples/` | Runnable root-crate experiments and benchmarks |
| `web/` | Browser-native 2D/3D replay clients embedded in the Rust binary, plus Node tests |
| `site/` | Separate React/Vinext public product site and its tests |
| `docs/` | Supporting documentation and version-qualified reference map |
| `design/` | Source visual references for the field-journal interface |
| `.github/` | CI, secret scanning, and dependency-update configuration |

## Invariants

- The Rust simulation and evaluation code is authoritative. Browser surfaces may project or approximate replay data but must not silently redefine fitness, evolution, archive, or checkpoint semantics.
- A fixed seed must produce identical results within one binary. Cross-architecture rigid-body goldens assert behavioral bands rather than bit-identical trajectories.
- Checkpoints contain versioned experiment state and are written by atomic replacement. Unknown versions and malformed checkpoint data must fail explicitly.
- The Rust GUI is an unauthenticated, single-user local tool. Keep its default loopback binding unless an authenticated TLS boundary is deliberately supplied.
- `axiom-field` parameters and software-only holdout results are uncalibrated. Do not present them as hardware validation or measured sim-to-real transfer.

## Conventions

- Use Conventional Commit prefixes such as `feat:`, `fix:`, `test:`, `docs:`, `ci:`, `refactor:`, and `chore:`.
- Work on a focused branch and use a pull request; CI runs for pull requests and pushes to `main`.
- Keep generated build output, local checkpoints, environment files, and release bundles out of version control according to `.gitignore`.
- The project is licensed under the MIT License in `LICENSE`.

## Production readiness

The standing to-do list every project in the fleet answers. Unchecked means *not
done yet* — not "not applicable". If a line genuinely does not apply to this
repo, say so on the line rather than deleting it, so the set stays comparable
across projects. Tick a box only when the thing it names exists in this repo and
is enforced by CI or documented above; an untrue tick is padding, and graders
catch padding.

- [ ] **System design** — the problem, the users, the constraints and the non-goals are written down, along with the trade-off each major decision accepted.
- [ ] **System architecture** — components, their boundaries and the data flowing between them are documented, including every external dependency and what happens when it is unavailable.
- [ ] **Frontend** — state ownership, routing, and the loading / empty / error states are deliberate; accessibility and responsive behaviour are verified, not assumed.
- [ ] **APIs and backend logic** — every endpoint has a versioned contract, validated input and typed errors, with business logic kept out of the transport handlers.
- [ ] **Databases and storage** — schema changes ship as migrations, indexes match the queries actually run, and a restore from backup has been rehearsed at least once.
- [ ] **Auth and permissions** — authentication and authorization are enforced server-side on every route and resource, credentials/sessions expire, and roles carry least privilege.
- [ ] **Hosting and cloud** — the deploy target, runtime configuration and secret storage are documented, and an environment can be reproduced from this repo alone.
- [ ] **CI/CD and version control** — lint, type-check, test and dependency audit run on every PR; releases are tagged; work lands through branch + PR, never a direct push to main.
- [ ] **Security** — no secrets in history, input validated at the boundaries, parameterised queries only, dependency audit blocking in CI, and a SECURITY.md with a disclosure route.
- [ ] **Rate limiting** — public and expensive endpoints carry per-identity quotas, with defined limits, a documented response when they are exceeded, and client backoff.
- [ ] **Caching and CDN** — cache layers and TTLs are chosen deliberately rather than defaulted, static assets are served from a CDN, and the invalidation path is written down.
- [ ] **Error tracking and logs** — logs are structured and correlated by request, unhandled errors report to one place, and neither secrets nor PII reach the log output.
- [ ] **Monitoring and alerts** — a health check plus the few metrics that actually matter, alerting on user-visible symptoms rather than causes, with a named owner for each alert.
- [ ] **Testing** — real assertions on real behaviour, run in CI behind a coverage floor that fails the build, and an end-to-end test over the critical path.
- [ ] **Scaling** — the current bottleneck is named and measured, timeouts and load-shedding are set, and the next scaling step is written down before it is needed.

## Before you finish

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo llvm-cov --workspace --fail-under-lines 80`
- [ ] `for file in web/*.js; do node --input-type=module --check < "$file"; done`
- [ ] `node --test web/tests/*.test.mjs`
- [ ] `cargo run --locked -- animate cpg --frames 3 --fps 0 --no-clear --width 60 --height 18`
- [ ] `cargo run --locked -- gui --check`
- [ ] `cargo package --locked`
- [ ] `cargo audit`
- [ ] `gitleaks git --no-banner --redact`
- [ ] `npm --prefix site run lint`
- [ ] `npm --prefix site test`
- [ ] `python -m unittest discover -s tests_python -p 'test_*.py' -v`
