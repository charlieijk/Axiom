"""Public wrapper contracts for the documented Axiom command surface."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "bin" / "axiom"


@unittest.skipUnless(os.name == "posix", "the Axiom wrapper is a POSIX shell script")
class AxiomCliTests(unittest.TestCase):
    def test_help_lists_the_candidate_handoff_workflow(self) -> None:
        result = subprocess.run(
            [str(CLI), "help"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("handoff CHECKPOINT --output PATH", result.stdout)

    def test_handoff_commands_forward_to_the_rust_binary(self) -> None:
        for command in ("handoff", "shortlist"):
            with self.subTest(command=command), tempfile.TemporaryDirectory(
                prefix="axiom-cli-"
            ) as temporary:
                directory = Path(temporary)
                fake_bin = directory / "bin"
                fake_bin.mkdir()
                record = directory / "cargo-args.txt"
                cargo = fake_bin / "cargo"
                cargo.write_text(
                    "#!/usr/bin/env bash\n"
                    "set -euo pipefail\n"
                    "printf '%s\\n' \"$@\" > \"$AXIOM_CLI_RECORD\"\n",
                    encoding="utf-8",
                )
                cargo.chmod(0o700)

                environment = os.environ.copy()
                environment.update(
                    {
                        "AXIOM_CLI_RECORD": str(record),
                        "PATH": str(fake_bin) + os.pathsep + environment["PATH"],
                    }
                )
                result = subprocess.run(
                    [
                        str(CLI),
                        command,
                        "checkpoints/run.json",
                        "--output",
                        "exports/shortlist.json",
                    ],
                    cwd=ROOT,
                    env=environment,
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=10,
                )

                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    record.read_text(encoding="utf-8").splitlines(),
                    [
                        "run",
                        "--locked",
                        "--",
                        command,
                        "checkpoints/run.json",
                        "--output",
                        "exports/shortlist.json",
                    ],
                )


if __name__ == "__main__":
    unittest.main()
