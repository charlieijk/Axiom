import hashlib
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
PACKAGER = ROOT / "script" / "package_release.sh"


class ReleasePackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.temporary_root = Path(self.temporary_directory.name)
        self.binary = self.temporary_root / "axiom"
        self.binary.write_text(
            "#!/bin/sh\n"
            'if [ "${1:-}" = "--version" ]; then\n'
            "  printf 'axiom 1.2.3\\n'\n"
            "  exit 0\n"
            "fi\n"
            "exit 2\n"
        )
        self.binary.chmod(0o755)
        self.output = self.temporary_root / "release"

    def package(self, version="v1.2.3", target="aarch64-apple-darwin"):
        return subprocess.run(
            [
                str(PACKAGER),
                str(self.binary),
                version,
                target,
                str(self.output),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_archive_contains_the_executable_runtime_docs_and_licenses(self):
        result = self.package()

        self.assertEqual(result.returncode, 0, result.stderr)
        archive_name = "axiom-v1.2.3-aarch64-apple-darwin"
        archive = self.output / f"{archive_name}.tar.gz"
        checksum = self.output / f"{archive_name}.tar.gz.sha256"
        self.assertTrue(archive.is_file())
        self.assertTrue(checksum.is_file())

        with tarfile.open(archive, "r:gz") as bundle:
            members = {
                member.name: member for member in bundle.getmembers() if member.isfile()
            }
            expected = {
                f"{archive_name}/axiom",
                f"{archive_name}/INSTALL.md",
                f"{archive_name}/LICENSE",
                f"{archive_name}/README.md",
                f"{archive_name}/SECURITY.md",
                f"{archive_name}/THIRD_PARTY_LICENSES/MANROPE-LICENSE.txt",
                f"{archive_name}/THIRD_PARTY_LICENSES/NEWSREADER-LICENSE.txt",
                f"{archive_name}/THIRD_PARTY_LICENSES/PHOSPHOR-LICENSE.txt",
                f"{archive_name}/THIRD_PARTY_LICENSES/THREE-LICENSE.txt",
            }
            self.assertEqual(set(members), expected)
            self.assertNotEqual(members[f"{archive_name}/axiom"].mode & 0o111, 0)
            packaged_binary = bundle.extractfile(f"{archive_name}/axiom")
            self.assertIsNotNone(packaged_binary)
            self.assertEqual(packaged_binary.read(), self.binary.read_bytes())

        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.assertEqual(checksum.read_text(), f"{digest}  {archive.name}\n")

    def test_packager_rejects_unsafe_names_and_existing_artifacts(self):
        unsafe = self.package(version="latest", target="../escape")
        self.assertEqual(unsafe.returncode, 2)
        self.assertFalse(self.output.exists())

        first = self.package()
        second = self.package()
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 1)
        self.assertIn("refusing to overwrite", second.stderr)

    def test_packager_rejects_a_binary_from_another_version(self):
        result = self.package(version="v1.2.4")

        self.assertEqual(result.returncode, 1)
        self.assertIn("reported 'axiom 1.2.3'; expected 'axiom 1.2.4'", result.stderr)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
