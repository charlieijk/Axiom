# Querying Axiom runs with DuckDB

Axiom persists a run as a versioned JSON checkpoint (`axiom evolve --save PATH`),
and that stays the source of truth — it is what `axiom inspect` and `axiom
animate --checkpoint` read, and this document changes nothing about it. What is
added here is a **read/analytics layer**: a way to put many runs side by side and
ask questions across seeds and budgets that a single checkpoint cannot answer.

Everything below was executed against checkpoints produced by this repository's
own binary. Nothing is illustrative.

## Setup

```sh
pip install -r requirements-duckdb.txt
```

## The short version

```sh
# Produce some runs (the checkpoint format is unchanged).
for seed in 1 2 3; do
  for gens in 5 10; do
    cargo run --release -- evolve --seed "$seed" --generations "$gens" \
      --population 12 --steps 60 --save "checkpoints/seed${seed}-gen${gens}.json"
  done
done

# Lift the analytical columns out into Parquet.
python script/export_checkpoints_parquet.py
# -> exports/run_generations.parquet: 45 rows
# -> exports/run_archive_cells.parquet: 18 rows
```

## Reading checkpoints directly (no export)

DuckDB reads the checkpoint JSON natively — the pretty-printed, multi-line single
object needs no preprocessing — so for a one-off question you do not need the
export at all:

```sql
SELECT
    config.seed                      AS seed,
    generation_summary.generation    AS generation,
    generation_summary.best_fitness  AS best_fitness,
    generation_summary.archive_coverage
FROM read_json_auto('checkpoints/*.json', filename = true) AS checkpoint,
     UNNEST(checkpoint.report.history) AS unnested(generation_summary)
ORDER BY seed, generation;
```

Use this when you are exploring one or two runs.

## Why the Parquet export exists

A checkpoint nests the per-generation quality-diversity series *underneath* the
full winning genome and the full archive — every elite carries its complete body
graph and network weights. Reaching a few dozen numbers therefore means parsing
megabytes of genome. Measured on the six runs above (920 KB of checkpoint JSON),
computing per-seed QD-score:

| path | best-of-7 time | bytes on disk |
| --- | --- | --- |
| `read_json_auto` over checkpoints | 14-17 ms | 928,471 |
| `read_parquet` over the export | 0.4 ms | 10,421 |

Roughly 40x faster and 89x smaller, returning identical values. The byte counts
are exact and reproducible; the timings are machine-dependent and varied by a few
milliseconds across repeat measurements, so treat the ratio as an order of
magnitude rather than a benchmark. The gap widens with run count, because the
JSON path re-parses every genome on every query. Export once, then query freely.

## What the export contains

`script/export_checkpoints_parquet.py` writes two tidy tables into `exports/`.
Both carry the run's configuration on every row, so no join back to the JSON is
needed to compare across seeds and budgets.

**`run_generations.parquet`** — one row per (run, generation): the QD
progression. Columns: `run_id`, `seed`, `budget_generations`, `population_size`,
`evaluation_steps`, `task`, `search_mode`, `archive_width`, `archive_height`,
`archive_cells`, `generation`, `best_fitness`, `mean_fitness`,
`archive_coverage`, `occupied_cells`, `run_evaluated_count`, `saved_at_unix_ms`.

**`run_archive_cells.parquet`** — one row per occupied cell of the run's final
archive. Columns: `run_id`, `seed`, `budget_generations`, `task`,
`archive_x_axis`, `archive_y_axis`, `archive_width`, `archive_height`,
`archive_cells`, `cell_x`, `cell_y`, `fitness`, `evaluation_steps`,
`discovered_generation`, `genome_id`, `parent_id`, `mutation_summary`, `controller`, and the behaviour metrics
`distance`, `stable_distance`, `jump_height`, `uprightness`, `stability`,
`body_count`, `actuator_count`, `energy`.

`run_id` is the full checkpoint path returned by DuckDB for the input glob. Both
exports use that same value as their join/group key, so equal basenames in
different experiment directories remain distinct runs.

### On QD-score

QD-score is **not** a stored column. `GenerationSummary` records `best_fitness`,
`mean_fitness`, `archive_coverage`, and `occupied_cells` — not the sum of elite
fitness — so a *per-generation* QD-score is not recoverable from a checkpoint and
this export does not invent one. The run's **final** QD-score is recoverable, as
`SUM(fitness)` over `run_archive_cells`, and that is what the query below uses.

Fitness can be negative, so QD-score is not bounded below by the best cell.

## Worked example: comparing seeds across generation budgets

The question this layer exists for — *does doubling the generation budget buy
more than seed noise?* — needs every run in one place:

```sql
WITH per_run AS (
    SELECT
        run_id,
        seed,
        budget_generations,
        sum(fitness)         AS qd_score,
        count(*)             AS occupied_cells,
        max(fitness)         AS best_fitness,
        max(archive_cells)   AS archive_cells
    FROM read_parquet('exports/run_archive_cells.parquet')
    GROUP BY run_id, seed, budget_generations
)
SELECT
    budget_generations                                  AS budget,
    count(*)                                            AS seeds,
    round(avg(qd_score), 2)                             AS mean_qd_score,
    round(stddev_samp(qd_score), 2)                     AS sd_qd_score,
    round(avg(occupied_cells), 2)                       AS mean_occupied,
    round(avg(occupied_cells) / max(archive_cells), 4)  AS mean_coverage,
    round(avg(best_fitness), 2)                         AS mean_best_fitness,
    round(max(best_fitness), 2)                         AS max_best_fitness
FROM per_run
GROUP BY budget_generations
ORDER BY budget_generations;
```

Actual output over seeds 1-3 at budgets 5 and 10 (population 12, 60 steps,
`RoughTerrain`, 12x8 archive):

| budget | seeds | mean_qd_score | sd_qd_score | mean_occupied | mean_coverage | mean_best_fitness | max_best_fitness |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 5 | 3 | 41.35 | 11.14 | 2.33 | 0.0243 | 38.69 | 40.81 |
| 10 | 3 | 128.27 | 45.22 | 3.67 | 0.0382 | 43.78 | 46.84 |

Doubling the budget roughly triples mean QD-score while mean best fitness moves
only 38.69 -> 43.78: the extra generations are buying **coverage**, not a better
single solution, which is exactly the distinction QD-score exists to make.

Note the standard deviation is large relative to the gap (45.22 on a mean of
128.27), so three seeds cannot settle this on their own. Check the per-run rows
before believing the aggregate:

```sql
SELECT seed, budget_generations AS budget, count(*) AS occupied,
       round(sum(fitness), 2) AS qd_score, round(max(fitness), 2) AS best
FROM read_parquet('exports/run_archive_cells.parquet')
GROUP BY seed, budget_generations
ORDER BY seed, budget;
```

| seed | budget | occupied | qd_score | best |
| --- | --- | --- | --- | --- |
| 1 | 5 | 2 | 49.73 | 40.81 |
| 1 | 10 | 4 | 138.84 | 44.66 |
| 2 | 5 | 2 | 28.70 | 37.21 |
| 2 | 10 | 3 | 78.71 | 39.85 |
| 3 | 5 | 3 | 45.62 | 38.05 |
| 3 | 10 | 4 | 167.27 | 46.84 |

Here the aggregate holds up: QD-score rises with budget in **every** seed
individually, not just on average.

## Joining the two tables

Per-generation progression and final archive share `run_id`:

```sql
SELECT g.run_id, g.seed, g.generation, g.best_fitness, g.archive_coverage,
       a.final_qd_score
FROM read_parquet('exports/run_generations.parquet') AS g
JOIN (
    SELECT run_id, sum(fitness) AS final_qd_score
    FROM read_parquet('exports/run_archive_cells.parquet')
    GROUP BY run_id
) AS a USING (run_id)
ORDER BY g.seed, g.budget_generations, g.generation;
```

## Schema drift

The checkpoint's `version` field does **not** discriminate between shapes. Legacy
`checkpoints/run-*.json` files in some working copies carry `version: 1` — the
same value the current writer stamps — but store the per-generation series under
`generation_summaries` instead of `history`. `axiom inspect` rejects them, and so
does the export, loudly:

```text
export failed: Binder Error: Could not find key "history" in struct
Candidate Entries: "best_genome", "generation_summaries"
```

That is the intended behaviour: an unreadable checkpoint aborts the export rather
than producing a Parquet file full of nulls.

Two gates protect the queries in this document:

- `cargo test` runs
  `checkpoint::tests::checkpoint_json_exposes_the_columns_the_parquet_export_reads`,
  which asserts every JSON path the export addresses by name still serializes,
  and `archive_cells_serialize_as_a_dense_nullable_grid`, which pins the
  null-padded `width * height` cell array that the QD-score summation relies on.
  A `#[serde(rename = ...)]` breaks these even though the JSON round-trip test
  still passes.
- `python -m unittest discover -s tests_python` runs
  `tests_python/test_duckdb_export.py`, which generates real checkpoints with the
  repository's binary, exports them, pins both Parquet schemas (names *and*
  types, in order), cross-validates the exported archive against the
  `occupied_cells` and `archive_coverage` the run itself recorded, and asserts a
  renamed field aborts the export. It skips when duckdb or cargo is unavailable.

If you rename a serialized field on purpose, update the exporter query, this
document, and both tests together.

## What this is not

DuckDB is an embedded single-writer OLAP engine. It is a query layer over
exported results here, not a replacement for the checkpoint format — checkpoints
remain the resumable, replayable record of a run, and the exporter only ever
reads them.
