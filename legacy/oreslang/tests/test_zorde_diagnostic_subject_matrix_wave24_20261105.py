"""Adversarial exact-subject denial evidence matrix; never compiler attestation."""
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
SPEC = importlib.util.spec_from_file_location("zorde_check_under_test", SOURCE)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class ZordeSubjectMatrixWave24(unittest.TestCase):
    def check_storage(self, data, expected):
        self.assertEqual(CHECK.valid_log(data, "private-storage-access"), expected)

    def test_canonical_private_member_denial(self):
        self.check_storage(b"error: entries is private\n", True)

    def test_canonical_access_denied_quoted_subject(self):
        self.check_storage(b"error: access denied to 'entries'\n", True)

    def test_canonical_private_compiled_type_denial(self):
        self.assertTrue(CHECK.valid_log(
            b"error: private class ZordePrivateCompiledEntry\n", "private-entry-import"))

    def test_other_private_subject_is_not_evidence(self):
        self.check_storage(b"error: private field other; note: entries exists\n", False)

    def test_prefix_and_suffix_identifier_spoofing_denied(self):
        self.check_storage(b"error: reentries is private\n", False)
        self.check_storage(b"error: entries2 is private\n", False)
        self.check_storage(b"error: entries_shadow is private\n", False)

    def test_nbsp_between_subject_and_denial_is_not_ascii_space(self):
        self.check_storage("entries\u00a0is private".encode("utf-8"), False)

    def test_line_spliced_denial_is_not_single_diagnostic(self):
        self.check_storage(b"error: entries\nis private\n", False)

    def test_case_insensitive_denial_keyword_is_accepted(self):
        self.check_storage(b"error: entries IS PRIVATE\n", True)

    def test_wrong_fixture_subject_is_not_accepted(self):
        self.assertFalse(CHECK.valid_log(
            b"error: entries is private\n", "private-entry-import"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
