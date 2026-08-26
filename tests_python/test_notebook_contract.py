"""Static contracts for the checked-in Colab notebook."""

from __future__ import annotations

import json
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
NOTEBOOK = REPOSITORY_ROOT / "notebooks" / "01_quality_diversity.ipynb"


class NotebookContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        document = json.loads(NOTEBOOK.read_text(encoding="utf-8"))
        cls.document = document
        cls.code = "\n".join(
            "".join(cell.get("source", []))
            for cell in document["cells"]
            if cell.get("cell_type") == "code"
        )
        cls.markdown = "\n".join(
            "".join(cell.get("source", []))
            for cell in document["cells"]
            if cell.get("cell_type") == "markdown"
        )

    def test_public_clone_is_anonymous_and_uses_an_argument_vector(self) -> None:
        self.assertNotIn("GITHUB_TOKEN", self.code)
        self.assertNotIn("GIT_ASKPASS", self.code)
        self.assertNotIn("getpass", self.code)
        self.assertIn('["git", "clone"', self.code)
        self.assertIn('f"https://github.com/{REPO}.git"', self.code)

    def test_public_clone_failure_is_explicit(self) -> None:
        self.assertIn("capture_output=True", self.code)
        self.assertIn('raise RuntimeError(f"git clone failed ({clone.returncode})")', self.code)
        self.assertIn("clones it anonymously over HTTPS", self.markdown)

    def test_notebook_has_no_saved_outputs(self) -> None:
        for cell in self.document["cells"]:
            if cell.get("cell_type") == "code":
                self.assertIsNone(cell.get("execution_count"))
                self.assertEqual(cell.get("outputs"), [])

    def test_every_cell_has_a_unique_nbformat_45_identifier(self) -> None:
        self.assertEqual(self.document["nbformat"], 4)
        self.assertGreaterEqual(self.document["nbformat_minor"], 5)
        identifiers = [cell.get("id") for cell in self.document["cells"]]
        self.assertTrue(all(isinstance(identifier, str) and identifier for identifier in identifiers))
        self.assertEqual(len(identifiers), len(set(identifiers)))

    def test_result_provenance_uses_full_argv_safe_git_identities(self) -> None:
        self.assertIn('subprocess.run(["git", *args]', self.code)
        self.assertIn('"commit": git("rev-parse", "HEAD")', self.code)
        self.assertIn('"tree": git("rev-parse", "HEAD^{tree}")', self.code)
        self.assertIn('"dirty_worktree": bool(git("status", "--porcelain"))', self.code)
        provenance = next(
            "".join(cell.get("source", []))
            for cell in self.document["cells"]
            if cell.get("id") == "provenance-stamp"
        )
        self.assertNotIn("shell=True", provenance)
        self.assertNotIn("--short", provenance)


if __name__ == "__main__":
    unittest.main()
