#!/usr/bin/env python3
"""Source-free, synthetic refusal tests for approved serde guest launcher."""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LAUNCH = ROOT / "scripts/test-serde-source-local.sh"


class SerdeSourceLocalAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="serde-admission-host-only-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)
        self.compiler = self.dir / "synthetic-compiler"
        self.compiler.write_text("#!/bin/sh\nexit 42\n")
        self.compiler.chmod(0o700)
        self.evidence = self.dir / "private-evidence"
        self.evidence.mkdir()
        self.env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "GITHUB_ACTIONS": "false",
            "EXPECTED_SOURCE_SHA": "a" * 40,
            "EXPECTED_SOURCE_TREE": "b" * 40,
            "EXPECTED_COMPILER_SHA256": "0" * 64,
            "ORESLANG_BIN": str(self.compiler),
            "SERDE_EVIDENCE_ROOT": str(self.evidence),
        }

    def invoke(self):
        return subprocess.run(
            ["bash", str(LAUNCH)],
            env=self.env, text=True, capture_output=True, timeout=15,
            check=False,
        )

    def test_refuses_missing_immutable_source_approval(self):
        self.env.pop("EXPECTED_SOURCE_SHA")
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("missing immutable approved HEAD", result.stderr)

    def test_refuses_runner_in_wrong_repository(self):
        self.env.update(GITHUB_ACTIONS="true", GITHUB_REPOSITORY="untrusted/example")
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("runner is not in the approved source repository", result.stderr)

    def test_refuses_compiler_digest_mismatch_before_guest_execution(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("compiler digest mismatch", result.stderr)
        self.assertEqual(list(self.evidence.iterdir()), [])

    def test_refuses_evidence_directory_inside_source_tree(self):
        self.env["SERDE_EVIDENCE_ROOT"] = str(ROOT)
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("evidence must remain outside source checkout", result.stderr)

    def test_refuses_relative_compiler_path(self):
        self.env["ORESLANG_BIN"] = "synthetic-compiler"
        result = self.invoke()
        self.assertEqual(result.returncode, 2)
        self.assertIn("compiler path must be absolute", result.stderr)


if __name__ == "__main__":
    unittest.main()
