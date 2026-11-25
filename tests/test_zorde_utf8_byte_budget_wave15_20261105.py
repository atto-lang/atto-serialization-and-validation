"""UTF-8 byte-oriented exact-subject evidence tests; no native guest certification."""
import importlib.util
from pathlib import Path
import unittest

source = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
spec = importlib.util.spec_from_file_location("zorde_denial_under_test", source)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)

SUBJECT = b"error: entries is private"
FIXTURE = "private-storage-access"


class ZordeUtf8Boundaries(unittest.TestCase):
    def test_exact_4096_byte_line_can_supply_evidence(self):
        line = SUBJECT + b" " * (4096 - len(SUBJECT))
        self.assertEqual(len(line), 4096)
        self.assertTrue(validator.valid_log(line, FIXTURE))

    def test_4097_byte_line_cannot_supply_evidence(self):
        line = SUBJECT + b" " * (4097 - len(SUBJECT))
        self.assertFalse(validator.valid_log(line, FIXTURE))

    def test_multibyte_unicode_cannot_bypass_line_byte_budget(self):
        base = SUBJECT.decode("utf-8")
        partial = base + "é" * ((4096 - len(SUBJECT)) // 2)
        exact = partial + "x" * (4096 - len(partial.encode("utf-8")))
        self.assertEqual(len(exact.encode("utf-8")), 4096)
        self.assertTrue(validator.valid_log(exact.encode("utf-8"), FIXTURE))
        self.assertFalse(validator.valid_log((exact + "x").encode("utf-8"), FIXTURE))

    def test_exact_overall_byte_budget_is_accepted_only_with_short_diagnostic(self):
        log = b"x" * (validator.MAX_LOG_BYTES - len(SUBJECT) - 1) + b"\n" + SUBJECT
        self.assertEqual(len(log), validator.MAX_LOG_BYTES)
        self.assertTrue(validator.valid_log(log, FIXTURE))
        self.assertFalse(validator.valid_log(log + b"x", FIXTURE))

    def test_denial_cannot_be_forged_by_splicing_lines(self):
        self.assertFalse(validator.valid_log(b"entries\nis private", FIXTURE))
        self.assertFalse(validator.valid_log("entries\u2028is private".encode("utf-8"), FIXTURE))

    def test_invalid_encoding_and_embedded_nul_are_denied(self):
        self.assertFalse(validator.valid_log(SUBJECT + b"\xff", FIXTURE))
        self.assertFalse(validator.valid_log(SUBJECT + b"\x00", FIXTURE))

    def test_unknown_fixture_cannot_claim_private_member_evidence(self):
        self.assertFalse(validator.valid_log(SUBJECT, "private-storage-access-typo"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
