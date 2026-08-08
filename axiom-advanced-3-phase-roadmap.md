# Axiom Advanced Roadmap

## Phase 1: Advanced Controller Stack

Completed in this rebuild foundation:

- `ControllerKind` with feedforward, recurrent, and CPG modes.
- Persistent `BrainState` for stateful rollout.
- Shared observation vector used by evaluator-facing code.
- Regression coverage around network input/bias ordering.
- Versioned controller and genome serialization inside experiment checkpoints.

Next expansion:

- richer recurrent topologies
- CPG parameter mutation separate from network weights

## Phase 2: Quality-Diversity Search

Completed in this rebuild foundation:

- MAP-Elites archive with configurable descriptor axes in code.
- Archive replacement by fitness per cell.
- Archive-driven parent sampling.
- Distance/body-count default descriptor grid.
- browser Archive Lab with live occupancy and fitness intensity
- direct replay selection for every occupied cell
- durable archive checkpoints with genome identity and parent lineage
- truthful lineage and mutation details for the selected browser elite

Next expansion:

- alternate descriptor-axis filters

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
