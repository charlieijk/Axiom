# Axiom Advanced Roadmap

## Phase 1: Advanced Controller Stack

Completed in this rebuild foundation:

- `ControllerKind` with feedforward, recurrent, and CPG modes.
- Persistent `BrainState` for stateful rollout.
- Shared observation vector used by evaluator-facing code.
- Regression coverage around network input/bias ordering.

Next expansion:

- richer recurrent topologies
- controller checkpoint serialization
- CPG parameter mutation separate from network weights

## Phase 2: Quality-Diversity Search

Completed in this rebuild foundation:

- MAP-Elites archive with configurable descriptor axes in code.
- Archive replacement by fitness per cell.
- Archive-driven parent sampling.
- Distance/body-count default descriptor grid.

Next expansion:

- browser Archive Lab
- direct cell filters
- replay any occupied cell
- lineage and mutation history per elite
- checkpoint persistence for archive state

## Phase 3: Multi-Task Curriculum

Completed in this rebuild foundation:

- flat, rough-terrain, and recovery task types.
- task-aware world setup.

Next expansion:

- obstacle tasks
- gap crossing
- pushing/carrying tasks
- randomized terrain batches
- curriculum scheduler with per-task reporting
