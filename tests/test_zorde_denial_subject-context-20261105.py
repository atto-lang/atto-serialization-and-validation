#!/usr/bin/env python3
"""Subject-aware access-denial log controls; host-only, not guest compilation."""
import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).resolve().parents[1] / 'scripts/check-zorde-denial.py'
spec = importlib.util.spec_from_file_location('zorde_subject_context', path)
denial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(denial)

class DiagnosticSubjectContext(unittest.TestCase):
    def test_real_private_entry_and_storage_denials(self):
        self.assertTrue(denial.valid_log(b"error: private class 'ZordePrivateCompiledEntry'\n", 'private-entry-import'))
        quote = bytes([96])
        self.assertTrue(denial.valid_log(b"error: cannot access field " + quote + b"entries" + quote + b"\n",
                                         'private-storage-access'))
        self.assertTrue(denial.valid_log(b"error: entries\tis private\n", 'private-storage-access'))

    def test_unrelated_denial_on_same_line_is_not_evidence(self):
        for text, fixture in [
            (b"error: Other is private; ZordePrivateCompiledEntry present", 'private-entry-import'),
            (b"error: private class Other ZordePrivateCompiledEntry", 'private-entry-import'),
            (b"error: other is private, entries mentioned", 'private-storage-access'),
            (b"warning: entries visible; Other is private", 'private-storage-access'),
        ]:
            with self.subTest(text=text):
                self.assertFalse(denial.valid_log(text, fixture))

    def test_cross_line_splices_and_suffixes_are_not_denials(self):
        self.assertFalse(denial.valid_log(b"error: entries\nis private", 'private-storage-access'))
        self.assertFalse(denial.valid_log(b"error: entriesHolder is private", 'private-storage-access'))
        self.assertFalse(denial.valid_log(b"error: ZordePrivateCompiledEntry2 is private", 'private-entry-import'))

    def test_unicode_lookalike_subjects_and_invalid_bytes(self):
        cyrillic_i = "error: entrіes is private".encode('utf-8')
        self.assertFalse(denial.valid_log(cyrillic_i, 'private-storage-access'))
        self.assertFalse(denial.valid_log(b'error: entries is private\xff', 'private-storage-access'))
        self.assertFalse(denial.valid_log(b'error: entries is private\x00', 'private-storage-access'))

    def test_entire_log_budget_and_unknown_fixture_fail_closed(self):
        self.assertFalse(denial.valid_log(b'error: entries is private', 'unknown-fixture'))
        self.assertFalse(denial.valid_log(b'error: entries is private\n' + b'x' * denial.MAX_LOG_BYTES,
                                          'private-storage-access'))
        self.assertTrue(denial.valid_log(b'warning: other is private\nerror: cannot access field entries\n',
                                         'private-storage-access'))

if __name__ == '__main__':
    unittest.main()
