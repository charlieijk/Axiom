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

    def test_private_clone_keeps_the_token_out_of_url_and_argv(self) -> None:
        self.assertNotIn("x-access-token:{token}", self.code)
        self.assertNotIn("token}@github.com", self.code)
        self.assertIn("GIT_ASKPASS", self.code)
        self.assertIn('["git", "clone"', self.code)
        self.assertIn("GIT_TERMINAL_PROMPT", self.code)

    def test_private_clone_does_not_surface_git_error_output(self) -> None:
        self.assertIn("capture_output=True", self.code)
        self.assertIn(
            "git clone failed; check the token scope and repository access", self.code
        )
        self.assertIn("arguments, command output, errors", self.markdown)

    def test_notebook_has_no_saved_outputs(self) -> None:
        document = json.loads(NOTEBOOK.read_text(encoding="utf-8"))
        for cell in document["cells"]:
            if cell.get("cell_type") == "code":
                self.assertIsNone(cell.get("execution_count"))
                self.assertEqual(cell.get("outputs"), [])


if __name__ == "__main__":
    unittest.main()
