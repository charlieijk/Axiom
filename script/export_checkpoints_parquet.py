#!/usr/bin/env python3
"""Export saved evolution checkpoints to Parquet for cross-run analysis.

The checkpoint format is unchanged and remains the source of truth: this reads
`checkpoints/*.json` and writes a flattened, columnar *copy* beside them. Nothing
here writes back into a checkpoint.

Why a copy at all, when DuckDB already reads the checkpoint JSON directly (see
`docs/duckdb.md`)? A checkpoint nests the per-generation quality-diversity
series underneath the full winning genome and the full archive, so every
`read_json_auto` of a run parses megabytes of genome and network weights to
reach a few dozen numbers. The export lifts the analytical columns out once:

  run_generations.parquet   one row per (run, generation) -- the QD progression
  run_archive_cells.parquet one row per (run, occupied archive cell) -- the final
                            archive, from which QD-score is a SUM

Both carry the run's configuration (seed, budget, task, archive shape) on every
row, so runs across seeds and budgets can be compared in one query without a
join back to the JSON.

Usage:
    python script/export_checkpoints_parquet.py [--checkpoints GLOB] [--out DIR]

Requires duckdb (`pip install -r requirements-duckdb.txt`).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent

# Every JSON path below is part of the checkpoint's serialized shape. Renaming a
# field in `EvolutionConfig`, `EvolutionReport`, `GenerationSummary`, `Elite`, or
# `Evaluation` breaks these queries, which is what
# `checkpoint_json_exposes_the_columns_the_parquet_export_reads` in
# src/checkpoint.rs and tests_python/test_duckdb_export.py exist to catch.

# One row per (run, generation). `history` is the per-generation series the
# search records as it runs; generation numbering starts at 1.
GENERATIONS_QUERY = """
SELECT
    filename                                          AS run_id,
    config.seed                                       AS seed,
    config.generations                                AS budget_generations,
    config.population_size                            AS population_size,
    config.evaluation_steps                           AS evaluation_steps,
    config.task                                       AS task,
    config.search_mode                                AS search_mode,
    config.archive_width                              AS archive_width,
    config.archive_height                             AS archive_height,
    config.archive_width * config.archive_height      AS archive_cells,
    generation_summary.generation                     AS generation,
    generation_summary.best_fitness                   AS best_fitness,
    generation_summary.mean_fitness                   AS mean_fitness,
    generation_summary.archive_coverage               AS archive_coverage,
    generation_summary.occupied_cells                 AS occupied_cells,
    checkpoint.report.evaluated_count                 AS run_evaluated_count,
    checkpoint.saved_at_unix_ms                       AS saved_at_unix_ms
FROM read_json_auto($checkpoint_glob, filename = true) AS checkpoint,
     UNNEST(checkpoint.report.history) AS unnested(generation_summary)
ORDER BY seed, budget_generations, run_id, generation
"""

# One row per occupied cell of the FINAL archive. Empty cells serialize as null
# entries in the flat `cells` array, so the WHERE clause drops them; `cell` is a
# two-element [x, y] array and DuckDB list indexing is 1-based.
#
# The per-generation QD-score is deliberately absent: `GenerationSummary` does
# not record it, so it is not recoverable from a checkpoint. Only the run's final
# QD-score is, as SUM(fitness) over these rows.
ARCHIVE_QUERY = """
SELECT
    filename                                          AS run_id,
    config.seed                                       AS seed,
    config.generations                                AS budget_generations,
    config.task                                       AS task,
    checkpoint.report.archive.x_axis                  AS archive_x_axis,
    checkpoint.report.archive.y_axis                  AS archive_y_axis,
    checkpoint.report.archive.width                   AS archive_width,
    checkpoint.report.archive.height                  AS archive_height,
    checkpoint.report.archive.width
        * checkpoint.report.archive.height             AS archive_cells,
    elite.cell[1]                                     AS cell_x,
    elite.cell[2]                                     AS cell_y,
    elite.evaluation.fitness                          AS fitness,
    elite.evaluation.steps                            AS evaluation_steps,
    elite.generation                                  AS discovered_generation,
    elite.genome_id                                   AS genome_id,
    elite.parent_id                                   AS parent_id,
    elite.mutation_summary                            AS mutation_summary,
    elite.genome.controller                           AS controller,
    elite.evaluation.metrics.distance                 AS distance,
    elite.evaluation.metrics.stable_distance          AS stable_distance,
    elite.evaluation.metrics.jump_height              AS jump_height,
    elite.evaluation.metrics.uprightness              AS uprightness,
    elite.evaluation.metrics.stability                AS stability,
    elite.evaluation.metrics.body_count               AS body_count,
    elite.evaluation.metrics.actuator_count           AS actuator_count,
    elite.evaluation.metrics.energy                   AS energy
FROM read_json_auto($checkpoint_glob, filename = true) AS checkpoint,
     UNNEST(checkpoint.report.archive.cells) AS unnested(elite)
WHERE elite IS NOT NULL
ORDER BY seed, budget_generations, run_id, cell_x, cell_y
"""

EXPORTS = (
    ("run_generations", GENERATIONS_QUERY),
    ("run_archive_cells", ARCHIVE_QUERY),
)


def export(checkpoint_glob: str, out_dir: Path) -> dict[str, int]:
    """Writes one Parquet file per export and returns each one's row count."""
    import duckdb

    out_dir.mkdir(parents=True, exist_ok=True)
    connection = duckdb.connect()
    written: dict[str, int] = {}

    for name, query in EXPORTS:
        destination = out_dir / f"{name}.parquet"
        # DuckDB has no parameter slot inside COPY's target path, so the
        # destination is interpolated as a quoted SQL literal while the glob
        # stays a bound parameter.
        escaped = str(destination).replace("'", "''")
        connection.execute(
            f"COPY ({query}) TO '{escaped}' (FORMAT PARQUET)",
            {"checkpoint_glob": checkpoint_glob},
        )
        written[name] = connection.execute(
            "SELECT count(*) FROM read_parquet(?)", [str(destination)]
        ).fetchone()[0]

    connection.close()
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--checkpoints",
        default=str(REPOSITORY_ROOT / "checkpoints" / "*.json"),
        help="glob of checkpoint files to export (default: checkpoints/*.json)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=REPOSITORY_ROOT / "exports",
        help="directory to write Parquet files into (default: exports/)",
    )
    arguments = parser.parse_args(argv)

    try:
        written = export(arguments.checkpoints, arguments.out)
    except ImportError:
        print(
            "duckdb is not installed; run: pip install -r requirements-duckdb.txt",
            file=sys.stderr,
        )
        return 2
    except Exception as error:  # noqa: BLE001 - surfaced verbatim to the operator
        # A checkpoint written by an older schema fails here rather than being
        # silently exported with null columns.
        print(f"export failed: {error}", file=sys.stderr)
        return 1

    if not any(written.values()):
        print(
            f"no checkpoint rows matched {arguments.checkpoints!r}; "
            "run `axiom evolve --save <path>` first",
            file=sys.stderr,
        )
        return 1

    for name, rows in written.items():
        print(f"{arguments.out / f'{name}.parquet'}: {rows} rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
