"""Contracts that keep public package metadata inside verified boundaries."""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class DocumentedClaimsTest(unittest.TestCase):
    def test_field_package_metadata_stays_software_only(self) -> None:
        with (ROOT / "crates" / "axiom-field" / "Cargo.toml").open("rb") as source:
            description = tomllib.load(source)["package"]["description"]

        self.assertEqual(
            description,
            "Rigid-body simulation of a fixed hobby-servo quadruped for software-only experiments.",
        )
        self.assertNotIn("sim-to-real", description.lower())
        self.assertNotIn("hardware validation", description.lower())


if __name__ == "__main__":
    unittest.main()
