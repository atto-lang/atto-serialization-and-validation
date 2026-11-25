"""Negative evidence controls against the real Zorde denial parser (host-only)."""
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
SPEC = importlib.util.spec_from_file_location("zorde_denial_wave31", SOURCE)
denial = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(denial)


class DenialEvidenceBoundary(unittest.TestCase):
    def test_actual_protected_subject_is_required(self):
        self.assertTrue(denial.valid_log(
            b"error: cannot access ZordePrivateCompiledEntry\n", "private-entry-import"))
        self.assertTrue(denial.valid_log(
            b"error: entries is private\n", "private-storage-access"))
        self.assertFalse(denial.valid_log(
            b"ZordePrivateCompiledEntry mentioned; cannot access PublicEntry\n",
            "private-entry-import"))

    def test_unknown_fixture_is_not_a_visibility_verdict(self):
        self.assertFalse(denial.valid_log(
            b"cannot access ZordePrivateCompiledEntry\n", "arbitrary-case"))

    def test_multibyte_diagnostic_line_uses_byte_budget(self):
        message = b"cannot access ZordePrivateCompiledEntry"
        self.assertTrue(denial.valid_log(
            b"\xc3\xa9" * 100 + b" " + message + b"\n", "private-entry-import"))
        self.assertFalse(denial.valid_log(
            b"\xc3\xa9" * 2048 + b" " + message + b"\n", "private-entry-import"))

    def test_invalid_bytes_and_nul_never_promote_diagnostic(self):
        for raw in (b"\xff cannot access ZordePrivateCompiledEntry\n",
                    b"cannot access ZordePrivateCompiledEntry\x00\n", b""):
            with self.subTest(raw=raw):
                self.assertFalse(denial.valid_log(raw, "private-entry-import"))

    def test_total_log_budget_is_not_overridden_by_valid_suffix(self):
        message = b"cannot access ZordePrivateCompiledEntry"
        self.assertFalse(denial.valid_log(
            b"x" * denial.MAX_LOG_BYTES + message, "private-entry-import"))


if __name__ == "__main__":
    unittest.main()
