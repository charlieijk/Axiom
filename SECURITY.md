# Security

## Reporting a vulnerability

Report suspected vulnerabilities privately by opening a
[GitHub security advisory](https://github.com/charlieijk/Axiom/security/advisories/new).
Please do not open a public issue for an unpatched problem.

## Trust boundary

`axiom gui` serves the replay browser and the 3D field journal for **one local
user**. It is unauthenticated by intent: there are no accounts, sessions, or
tokens, and every request that reaches the port is served.

That is safe because of where it listens. `GuiConfig::default()` binds
`127.0.0.1:8787`, so out of the box the operating system limits callers to this
machine. `axiom gui --host` can bind another interface, and doing so hands an
unauthenticated API to everyone who can route to it.

**Do not expose this server beyond the host.** It is a development and
demonstration tool, not a multi-tenant service. If you need it reachable from
another machine, put it behind something that terminates TLS and authenticates
the caller.

The API surface is read-only: `/api/replay` runs a simulation and returns JSON,
and nothing it accepts writes to disk or shells out. The one abuse worth naming
is cost rather than data — an `evolved` replay asks the server to run an
evolution loop — so requests whose
`generations * population * evaluation_steps` exceed the interactive budget are
rejected with `400` instead of being run.

## Supply chain

- `cargo audit` runs as its own CI job and fails on any known advisory in the
  dependency tree.
- The publishable `axiom` crate depends only on `serde` and `serde_json`; the
  heavier physics stack belongs to the `axiom-field` lab crate, which is
  `publish = false`.
- `Cargo.lock` is committed, and CI runs `cargo package --locked` so a release
  build resolves exactly what was tested.
