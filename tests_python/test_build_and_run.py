from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "script" / "build_and_run.sh"


@unittest.skipUnless(os.name == "posix", "the launcher is a POSIX shell script")
class BuildAndRunSecurityTests(unittest.TestCase):
    def _environment(self, directory: Path) -> dict[str, str]:
        fake_bin = directory / "bin"
        target = directory / "target"
        fake_bin.mkdir()
        cargo = fake_bin / "cargo"
        cargo.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            "mkdir -p \"$CARGO_TARGET_DIR/debug\"\n"
            "printf '#!/usr/bin/env bash\\nexit 0\\n' >\"$CARGO_TARGET_DIR/debug/axiom\"\n"
            "chmod 700 \"$CARGO_TARGET_DIR/debug/axiom\"\n",
            encoding="utf-8",
        )
        cargo.chmod(0o700)
        environment = os.environ.copy()
        environment.update(
            {
                "CARGO_TARGET_DIR": str(target),
                "CODEX_OPEN_BROWSER": "0",
                "PATH": str(fake_bin) + os.pathsep + environment["PATH"],
                "TMPDIR": str(directory),
            }
        )
        return environment

    def _run(self, directory: Path, mode: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(SCRIPT), mode],
            cwd=ROOT,
            env=self._environment(directory),
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_launcher_rejects_a_symlinked_runtime_directory(self) -> None:
        with tempfile.TemporaryDirectory(prefix="axiom-launcher-") as temporary:
            directory = Path(temporary)
            victim = directory / "victim"
            victim.mkdir()
            runtime = directory / f"axiom-codex-{os.getuid()}"
            runtime.symlink_to(victim, target_is_directory=True)

            result = self._run(directory, "--build-only")

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("symlinked Axiom runtime directory", result.stderr)
            self.assertEqual(list(victim.iterdir()), [])

    def test_launcher_rejects_symlinked_runtime_files_without_touching_victim(
        self,
    ) -> None:
        for file_name in ("service.log", "service.pid"):
            with self.subTest(file_name=file_name), tempfile.TemporaryDirectory(
                prefix="axiom-launcher-"
            ) as temporary:
                directory = Path(temporary)
                runtime = directory / f"axiom-codex-{os.getuid()}"
                runtime.mkdir(mode=0o700)
                victim = directory / "victim.txt"
                victim.write_text("do not replace", encoding="utf-8")
                (runtime / file_name).symlink_to(victim)

                result = self._run(directory, "--build-only")

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("symlinked Axiom", result.stderr)
                self.assertEqual(
                    victim.read_text(encoding="utf-8"),
                    "do not replace",
                )

    def test_launcher_privatises_runtime_directory_and_creates_regular_files(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(prefix="axiom-launcher-") as temporary:
            directory = Path(temporary)
            runtime = directory / f"axiom-codex-{os.getuid()}"
            runtime.mkdir(mode=0o700)
            runtime.chmod(0o777)

            result = self._run(directory, "run")

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(stat.S_IMODE(runtime.stat().st_mode), 0o700)
            for file_name in ("service.log", "service.pid"):
                path = runtime / file_name
                self.assertFalse(path.is_symlink())
                self.assertTrue(path.is_file())
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertRegex(
                (runtime / "service.pid").read_text(encoding="utf-8"),
                r"^\d+\n$",
            )


if __name__ == "__main__":
    unittest.main()
