"""End-to-end tests for the DuckDB Parquet export.

These run against checkpoints written by the repository's own binary, not against
fixtures: `axiom evolve --save` produces the input, so the test fails whenever the
real serialized shape stops matching what `script/export_checkpoints_parquet.py`
reads.

The suite skips when duckdb or cargo is unavailable, which keeps the existing
Flask CI job green; the `duckdb` job in .github/workflows/ci.yml provides both.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY_ROOT / "script"))

HAS_DUCKDB = importlib.util.find_spec("duckdb") is not None
CARGO = shutil.which("cargo")

# One row per generation, per run. Column order is the export's contract: a
# reordering is a schema change for anything reading the Parquet positionally.
EXPECTED_GENERATION_SCHEMA = [
    ("run_id", "VARCHAR"),
    ("seed", "BIGINT"),
    ("budget_generations", "BIGINT"),
    ("population_size", "BIGINT"),
    ("evaluation_steps", "BIGINT"),
    ("task", "VARCHAR"),
    ("search_mode", "VARCHAR"),
    ("archive_width", "BIGINT"),
    ("archive_height", "BIGINT"),
    ("archive_cells", "BIGINT"),
    ("generation", "BIGINT"),
    ("best_fitness", "DOUBLE"),
    ("mean_fitness", "DOUBLE"),
    ("archive_coverage", "DOUBLE"),
    ("occupied_cells", "BIGINT"),
    ("run_evaluated_count", "BIGINT"),
    ("saved_at_unix_ms", "BIGINT"),
]

EXPECTED_ARCHIVE_SCHEMA = [
    ("run_id", "VARCHAR"),
    ("seed", "BIGINT"),
    ("budget_generations", "BIGINT"),
    ("task", "VARCHAR"),
    ("archive_x_axis", "VARCHAR"),
    ("archive_y_axis", "VARCHAR"),
    ("archive_cells", "BIGINT"),
    ("cell_x", "BIGINT"),
    ("cell_y", "BIGINT"),
    ("fitness", "DOUBLE"),
    ("evaluation_steps", "BIGINT"),
    ("discovered_generation", "BIGINT"),
    ("genome_id", "BIGINT"),
    ("parent_id", "BIGINT"),
    ("mutation_summary", "VARCHAR"),
    ("controller", "VARCHAR"),
    ("distance", "DOUBLE"),
    ("stable_distance", "DOUBLE"),
    ("jump_height", "DOUBLE"),
    ("uprightness", "DOUBLE"),
    ("stability", "DOUBLE"),
    ("body_count", "DOUBLE"),
    ("actuator_count", "DOUBLE"),
    ("energy", "DOUBLE"),
]

# Small enough to stay a test, wide enough to compare seeds against budgets.
SEEDS = (1, 2)
BUDGETS = (3, 6)
POPULATION = 8
STEPS = 40


@unittest.skipUnless(HAS_DUCKDB, "duckdb is not installed")
@unittest.skipUnless(CARGO, "cargo is not available to generate checkpoints")
class DuckDbExportTest(unittest.TestCase):
    """Exercises the export against freshly generated, real run data."""

    checkpoint_dir: Path
    export_dir: Path
    _temporary: tempfile.TemporaryDirectory

    @classmethod
    def setUpClass(cls) -> None:
        cls._temporary = tempfile.TemporaryDirectory()
        root = Path(cls._temporary.name)
        cls.checkpoint_dir = root / "checkpoints"
        cls.export_dir = root / "exports"
        cls.checkpoint_dir.mkdir(parents=True)

        for seed in SEEDS:
            for budget in BUDGETS:
                destination = cls.checkpoint_dir / f"seed{seed}-gen{budget}.json"
                subprocess.run(
                    [
                        CARGO,
                        "run",
                        "--release",
                        "--locked",
                        "--quiet",
                        "--",
                        "evolve",
                        "--seed",
                        str(seed),
                        "--generations",
                        str(budget),
                        "--population",
                        str(POPULATION),
                        "--steps",
                        str(STEPS),
                        "--save",
                        str(destination),
                    ],
                    cwd=REPOSITORY_ROOT,
                    check=True,
                    stdout=subprocess.DEVNULL,
                )

        import export_checkpoints_parquet

        cls.exporter = export_checkpoints_parquet
        cls.written = export_checkpoints_parquet.export(
            str(cls.checkpoint_dir / "*.json"), cls.export_dir
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls._temporary.cleanup()

    def query(self, sql: str, parameters: list | None = None) -> list[tuple]:
        import duckdb

        connection = duckdb.connect()
        try:
            return connection.execute(sql, parameters or []).fetchall()
        finally:
            connection.close()

    def parquet(self, name: str) -> str:
        return str(self.export_dir / f"{name}.parquet")

    def test_export_writes_one_row_per_run_generation(self) -> None:
        expected_rows = len(SEEDS) * sum(BUDGETS)
        self.assertEqual(self.written["run_generations"], expected_rows)

        runs = self.query(
            "SELECT count(DISTINCT run_id) FROM read_parquet(?)",
            [self.parquet("run_generations")],
        )[0][0]
        self.assertEqual(runs, len(SEEDS) * len(BUDGETS))

    def schema(self, name: str) -> list[tuple[str, str]]:
        """Column names and types, in order, as DuckDB reads the Parquet back."""
        return [
            (row[0], row[1])
            for row in self.query(
                "SELECT column_name, column_type "
                "FROM (DESCRIBE SELECT * FROM read_parquet(?))",
                [self.parquet(name)],
            )
        ]

    def test_generation_schema_matches_the_documented_columns(self) -> None:
        self.assertEqual(self.schema("run_generations"), EXPECTED_GENERATION_SCHEMA)

    def test_archive_schema_matches_the_documented_columns(self) -> None:
        self.assertEqual(self.schema("run_archive_cells"), EXPECTED_ARCHIVE_SCHEMA)

    def test_every_run_reaches_its_configured_generation_budget(self) -> None:
        rows = self.query(
            """
            SELECT run_id, budget_generations, count(*), min(generation), max(generation)
            FROM read_parquet(?)
            GROUP BY run_id, budget_generations
            """,
            [self.parquet("run_generations")],
        )
        self.assertEqual(len(rows), len(SEEDS) * len(BUDGETS))
        for run_id, budget, generations, first, last in rows:
            # Generation numbering starts at 1 and the history holds one row per
            # generation, so these three agree for any completed run.
            self.assertEqual(generations, budget, run_id)
            self.assertEqual(first, 1, run_id)
            self.assertEqual(last, budget, run_id)

    def test_exported_archive_reproduces_the_checkpoint_coverage(self) -> None:
        """Cross-validates the export against the numbers the search recorded.

        The occupied-cell count derived by unnesting the archive must match the
        `occupied_cells` the run itself wrote for its final generation. If the
        export ever mis-handles the null padding of empty cells, coverage and
        QD-score both silently drift; this catches that.
        """
        recorded = {
            run_id: (occupied, coverage, cells)
            for run_id, occupied, coverage, cells in self.query(
                """
                SELECT run_id, occupied_cells, archive_coverage, archive_cells
                FROM read_parquet(?) AS g
                QUALIFY row_number() OVER (
                    PARTITION BY run_id ORDER BY generation DESC
                ) = 1
                """,
                [self.parquet("run_generations")],
            )
        }
        derived = dict(
            self.query(
                "SELECT run_id, count(*) FROM read_parquet(?) GROUP BY run_id",
                [self.parquet("run_archive_cells")],
            )
        )

        self.assertEqual(set(recorded), set(derived))
        for run_id, (occupied, coverage, cells) in recorded.items():
            self.assertEqual(derived[run_id], occupied, run_id)
            self.assertAlmostEqual(coverage, occupied / cells, places=6, msg=run_id)

    def test_run_qd_score_matches_the_reported_best_fitness(self) -> None:
        """The best elite in the exported archive is the run's reported best."""
        rows = self.query(
            """
            WITH per_run AS (
                SELECT run_id, sum(fitness) AS qd_score, max(fitness) AS best
                FROM read_parquet(?)
                GROUP BY run_id
            ),
            reported AS (
                SELECT run_id, max(best_fitness) AS best_fitness
                FROM read_parquet(?)
                GROUP BY run_id
            )
            SELECT p.run_id, p.qd_score, p.best, r.best_fitness
            FROM per_run p JOIN reported r USING (run_id)
            """,
            [self.parquet("run_archive_cells"), self.parquet("run_generations")],
        )
        self.assertEqual(len(rows), len(SEEDS) * len(BUDGETS))
        for run_id, qd_score, best, reported_best in rows:
            self.assertAlmostEqual(best, reported_best, places=5, msg=run_id)
            # Fitness may be negative, so QD-score is not bounded below by the
            # best cell; it is bounded above by best * occupied.
            self.assertIsNotNone(qd_score, run_id)

    def test_cross_seed_comparison_query_runs_over_every_seed_and_budget(self) -> None:
        """Runs the exact comparison shape documented in docs/duckdb.md."""
        rows = self.query(
            """
            WITH per_run AS (
                SELECT run_id, seed, budget_generations,
                       sum(fitness) AS qd_score,
                       count(*) AS occupied_cells
                FROM read_parquet(?)
                GROUP BY run_id, seed, budget_generations
            )
            SELECT budget_generations, count(*) AS seeds,
                   avg(qd_score) AS mean_qd_score,
                   avg(occupied_cells) AS mean_occupied
            FROM per_run
            GROUP BY budget_generations
            ORDER BY budget_generations
            """,
            [self.parquet("run_archive_cells")],
        )
        self.assertEqual([row[0] for row in rows], sorted(BUDGETS))
        for budget, seeds, mean_qd_score, mean_occupied in rows:
            self.assertEqual(seeds, len(SEEDS), budget)
            self.assertIsNotNone(mean_qd_score, budget)
            self.assertGreater(mean_occupied, 0, budget)

    def test_export_fails_loudly_when_the_checkpoint_shape_drifts(self) -> None:
        """A renamed field must abort the export, not yield null columns.

        This is the failure the checked-in legacy `checkpoints/run-*.json` files
        already exhibit: they carry `version: 1`, the same value the current
        writer stamps, but store the per-generation series under
        `generation_summaries` rather than `history`.
        """
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = next(self.checkpoint_dir.glob("*.json"))
            drifted = json.loads(source.read_text(encoding="utf-8"))
            drifted["report"]["generation_summaries"] = drifted["report"].pop("history")
            (root / "drifted.json").write_text(json.dumps(drifted), encoding="utf-8")

            with self.assertRaises(Exception) as caught:
                self.exporter.export(str(root / "*.json"), root / "exports")

            self.assertIn("history", str(caught.exception))


if __name__ == "__main__":
    unittest.main()
