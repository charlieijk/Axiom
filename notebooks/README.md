# Notebooks

Colab-executed notebooks that run **the real Axiom engine**, versioned here in git rather than
living in Drive. The Rust code is the authority; a notebook never reimplements the search, the
fitness function, or the archive in Python.

| Notebook | What it establishes |
| --- | --- |
| `01_quality_diversity.ipynb` | The engine builds and runs headlessly on Colab; archive coverage and QD-score over generations across 8 seeds, with the spread stated; how little an early run predicts a late one; the archive's structural coverage ceiling; and that a seed reproduces a byte-identical report. |

## Opening one on Colab

```
https://colab.research.google.com/github/charlieijk/Axiom/blob/main/notebooks/01_quality_diversity.ipynb
```

This repository is public. Opening and running the notebook requires no GitHub token or
private-repository authorization. Colab asks for GitHub authorization only when you save a copy
back; target a branch and open a PR.

**If you use more than one Google account, check which one you are on before authorizing.** The
GitHub grant attaches to a single Colab account. Open the notebook later under a different account
and the integration simply is not there — it reads as a broken setup when it is only the wrong
account. Colab identifies accounts with an `authuser=N` index in the URL, but that number is
*positional*, not stable: it renumbers when you sign in or out of accounts, so confirm the account
by its avatar rather than trusting the index. The durable fix is a dedicated browser profile for the
account you do this work in.

Set the runtime to **CPU**. Axiom's simulation is single-threaded Rust; a GPU would sit idle and
still bill you.

## GitHub access

Axiom is public. Opening the notebook and cloning the repository use anonymous HTTPS and require no
GitHub token or private-repository authorization. GitHub authentication is needed only when you use
Colab's save-to-GitHub flow; save to a branch and open a PR.

## The Rust toolchain on Colab

Colab has no Rust. The setup cell installs one with `rustup --profile minimal`, and **that download
is the slow part** — expect several minutes on a cold runtime. The build itself is fast: `axiom`
depends only on `serde` and `serde_json`, and a cold release build of the harness took 17.7 seconds
locally. If a cell looks stuck, it is rustup, not `cargo`.

Build only what you need. `cargo build --release -p axiom --example …` skips `crates/axiom-field`,
which pulls in `rapier3d` and is a much heavier build.

## Saving changes back

`File -> Save a copy in GitHub`, targeting a **branch, never `main`**, then open a PR as usual.

**Clear outputs before saving** (`Edit -> Clear all outputs`). Two reasons: committed outputs make
review diffs unreadable, and stale numbers embedded in a notebook read as current evidence to anyone
skimming it. Results worth keeping get written to `notebooks/results/` with a provenance stamp
attached — commit, dirty flag, toolchain, timestamp — not left in cell output.

## Why `notebooks/results/` is git-ignored

Generated results are deliberately not committed. A measurement is only meaningful next to the code
that produced it, and a results file committed once quietly becomes a stale claim the next time the
fitness function, the mutation operator, or the archive geometry changes. Re-run the notebook; that
is the point of it being reproducible.

If a specific run ever does need to be preserved (a figure in a write-up, say), commit it under a
name that states what it is, and keep the provenance block intact.

## Ground rules for anything added here

1. **Drive the real engine.** Shell out to the CLI or to a small harness under `examples/`; do not
   port the search or the fitness function to Python.
2. **Seed everything, and record the seed** in the output. Axiom is deterministic given a seed —
   check that rather than assuming it.
3. **Several seeds, with the spread stated.** One run is an anecdote. A mean with no spread beside
   it hides exactly the thing a reader needs in order to judge it.
4. **Do not add fields to `EvolutionReport` to make a notebook easier.** Its serialization is pinned
   by a digest in `tests/arm64_evidence.rs`; changing it invalidates recorded Arm64 evidence.
   Compute what you need in the harness or in the notebook instead.
5. **Stamp provenance** on every saved result: commit, dirty-worktree flag, toolchain, timestamp.
6. **Write down the limits** in the notebook, not in someone's memory — including which numbers the
   notebook measured and which it merely quoted from a stored benchmark.
