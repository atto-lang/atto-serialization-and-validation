#!/usr/bin/env python3
"""Host-only tamper tests for Git raw worktree provenance gate."""
import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "serde_raw_source_verifier", ROOT / "scripts/verify-serde-worktree.py"
)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class SerdeRawWorktreeGateTests(unittest.TestCase):
    def run_git(self, *args):
        return subprocess.check_output(
            ["git", "-C", str(self.root), *args], stderr=subprocess.PIPE
        ).decode().strip()

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="serde-raw-git-host-only-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.run_git("init", "-q")
        self.run_git("config", "user.email", "test@example.invalid")
        self.run_git("config", "user.name", "Source-Free Test")
        (self.root / "test.atto").write_bytes(b"define fnc test() -> void {}\n")
        (self.root / ".gitignore").write_text("scratch.tmp\n")
        self.run_git("add", ".")
        self.run_git("commit", "-qm", "synthetic seed")
        self.head = self.run_git("rev-parse", "HEAD")
        self.tree = self.run_git("rev-parse", "HEAD^{tree}")

    def verify(self):
        return gate.verify(self.root, self.head, self.tree)

    def test_approved_raw_tree_is_accepted_as_source_only(self):
        self.assertEqual(self.verify(), 2)

    def test_modified_worktree_blob_is_rejected(self):
        (self.root / "test.atto").write_bytes(b"changed bytes")
        with self.assertRaises(ValueError):
            self.verify()

    def test_staged_index_divergence_is_rejected(self):
        (self.root / "test.atto").write_bytes(b"changed bytes")
        self.run_git("add", "test.atto")
        with self.assertRaises(ValueError):
            self.verify()

    def test_untracked_file_is_rejected(self):
        (self.root / "not-tracked.atto").write_text("new")
        with self.assertRaises(ValueError):
            self.verify()

    def test_ignored_file_is_rejected(self):
        (self.root / "scratch.tmp").write_text("ignored but not safe")
        with self.assertRaises(ValueError):
            self.verify()

    def test_wrong_head_or_tree_is_rejected(self):
        with self.assertRaises(ValueError):
            gate.verify(self.root, "0" * 40, self.tree)
        with self.assertRaises(ValueError):
            gate.verify(self.root, self.head, "0" * 40)

    @unittest.skipUnless(hasattr(os, "symlink"), "requires symlink support")
    def test_tracked_symlink_is_rejected(self):
        (self.root / "shortcut").symlink_to("test.atto")
        self.run_git("add", "shortcut")
        self.run_git("commit", "-qm", "track symlink")
        self.head = self.run_git("rev-parse", "HEAD")
        self.tree = self.run_git("rev-parse", "HEAD^{tree}")
        with self.assertRaises(ValueError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
