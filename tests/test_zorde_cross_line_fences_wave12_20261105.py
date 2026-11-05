#!/usr/bin/env python3
"""Source-only regression tests for exact protected-symbol diagnostic boundaries."""
import importlib.util
from pathlib import Path
import unittest

source = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
spec = importlib.util.spec_from_file_location("zorde_denial_wave12", source)
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class CrossLineDenial(unittest.TestCase):
    def test_subject_and_private_predicate_cannot_cross_line_break(self):
        for separator in (b"\n", b"\r", b"\r\n",
                          "\u0085".encode("utf-8"), "\u2028".encode("utf-8")):
            with self.subTest(separator=separator):
                self.assertFalse(gate.valid_log(
                    b"error: entries" + separator + b"is private",
                    "private-storage-access"))

    def test_wrong_subject_with_unrelated_denial_is_not_evidence(self):
        self.assertFalse(gate.valid_log(
            b"error: entries was inspected\n"
            b"error: another_symbol is private\n",
            "private-storage-access"))

    def test_invalid_utf8_anywhere_invalidates_entire_log(self):
        self.assertFalse(gate.valid_log(
            b"error: entries is private\n" + b"\xff",
            "private-storage-access"))

    def test_quoted_exact_subject_after_unrelated_line_is_accepted(self):
        self.assertTrue(gate.valid_log(
            b"note: evaluating visibility\n"
            b"error: cannot access field \`entries\`\r\n",
            "private-storage-access"))

    def test_unknown_fixture_never_passes(self):
        self.assertFalse(gate.valid_log(
            b"error: entries is private\n", "unknown-fixture"))


if __name__ == "__main__":
    unittest.main()
