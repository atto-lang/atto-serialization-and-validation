#!/usr/bin/env python3
"""Exact diagnostic subject/UTF-8 boundaries; source-free, not compiler proof."""
import importlib.util
from pathlib import Path
import unittest

source = Path(__file__).resolve().parents[1] / 'scripts/check-zorde-denial.py'
spec = importlib.util.spec_from_file_location('zorde_denial', source)
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class DenialBoundary(unittest.TestCase):
    def test_exact_private_entry_and_field_accepted(self):
        self.assertTrue(gate.valid_log(b"error: private class 'ZordePrivateCompiledEntry'\r\n", 'private-entry-import'))
        self.assertTrue(gate.valid_log(b'error: cannot access field `entries`\n', 'private-storage-access'))

    def test_lookalike_suffixes_and_prefixes_rejected(self):
        for text, fixture in (
            (b'error: ZordePrivateCompiledEntry2 is private', 'private-entry-import'),
            (b'error: preZordePrivateCompiledEntry is private', 'private-entry-import'),
            (b'error: entries2 is private', 'private-storage-access'),
            (b'error: preentries is private', 'private-storage-access'),
        ):
            with self.subTest(text=text):
                self.assertFalse(gate.valid_log(text, fixture))

    def test_cross_line_diagnostic_splice_rejected(self):
        self.assertFalse(gate.valid_log(b'error: entries\nother is private', 'private-storage-access'))

    def test_invalid_unicode_and_embedded_nul_rejected(self):
        self.assertFalse(gate.valid_log(b'error: entries is private\xff', 'private-storage-access'))
        self.assertFalse(gate.valid_log(b'error: entries is private\x00', 'private-storage-access'))

    def test_byte_budget_and_line_budget_are_distinct(self):
        self.assertFalse(gate.valid_log(b'error: entries is private' + b'x'*4097, 'private-storage-access'))
        self.assertFalse(gate.valid_log(b'error: entries is private\n' + b'x'*gate.MAX_LOG_BYTES, 'private-storage-access'))


if __name__ == '__main__':
    unittest.main()
