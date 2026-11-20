"""Fail-closed lexical denial evidence on the real host checker.

These tests do not establish compiler identity or successful guest execution.
"""
import importlib.util
from pathlib import Path
import unittest

source = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
spec = importlib.util.spec_from_file_location("zorde_check_wave31", source)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ZordeDiagnosticLexicalWave31(unittest.TestCase):
    def test_direct_subject_denial_is_admitted(self):
        self.assertTrue(checker.valid_log(
            b"error: ZordePrivateCompiledEntry is private\n",
            "private-entry-import",
        ))
        self.assertTrue(checker.valid_log(
            b"error: cannot access ZordePrivateCompiledEntry\n",
            "private-entry-import",
        ))

    def test_unrelated_private_subject_and_suffix_alias_are_denied(self):
        for line in (
            b"error: OtherSymbol is private; mentioned ZordePrivateCompiledEntry\n",
            b"error: ZordePrivateCompiledEntryExtra is private\n",
            b"error: private field entriesSuffix\n",
        ):
            with self.subTest(line=line):
                fixture = "private-storage-access" if b"entriesSuffix" in line else "private-entry-import"
                self.assertFalse(checker.valid_log(line, fixture))

    def test_untrusted_fixture_name_or_malformed_utf8_is_not_evidence(self):
        self.assertFalse(checker.valid_log(b"entries is private\n", "../private-storage-access"))
        self.assertFalse(checker.valid_log(b"\xff entries is private\n", "private-storage-access"))
        self.assertFalse(checker.valid_log(b"entries is private\x00\n", "private-storage-access"))

    def test_byte_budget_rejects_unbounded_log_even_with_true_diagnostic(self):
        log = b"entries is private\n" + b"X" * checker.MAX_LOG_BYTES
        self.assertFalse(checker.valid_log(log, "private-storage-access"))


if __name__ == "__main__":
    unittest.main()
