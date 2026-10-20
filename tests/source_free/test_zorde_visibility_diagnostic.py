#!/usr/bin/env python3
"""Self-contained source-free adversarial verifier tests (not Oreslang CI)."""
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts' / 'zorde_visibility_diagnostic.py'
spec = importlib.util.spec_from_file_location('zorde_visibility_diagnostic', SCRIPT)
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


class DiagnosticContracts(unittest.TestCase):
    def verdict(self, text: str | bytes, fixture: str = 'private-entry-import') -> bool:
        b = text.encode() if isinstance(text, str) else text
        return verifier.assess(b, fixture)[0]

    def test_explicit_private_entry_denial(self):
        self.assertTrue(self.verdict('error: ZordePrivateCompiledEntry is private'))

    def test_explicit_unexported_entry(self):
        self.assertTrue(self.verdict('error: ZordePrivateCompiledEntry not exported'))

    def test_adjacent_line_denial(self):
        self.assertTrue(self.verdict('error: private type\n  ZordePrivateCompiledEntry'))

    def test_storage_denial(self):
        self.assertTrue(self.verdict('error: cannot access member entries', 'private-storage-access'))

    def test_ansi_diagnostic(self):
        self.assertTrue(self.verdict('\x1b[31merror: ZordePrivateCompiledEntry not public\x1b[0m'))

    def test_split_unrelated_lines_are_refused(self):
        self.assertFalse(self.verdict('error: private type OtherClass\nignored\nwarning: ZordePrivateCompiledEntry'))

    def test_generic_access_denial_is_refused(self):
        self.assertFalse(self.verdict('error: OtherClass private symbol'))

    def test_target_without_denial_is_refused(self):
        self.assertFalse(self.verdict('error: unknown symbol ZordePrivateCompiledEntry'))

    def test_syntax_error_mixed_with_denial_is_refused(self):
        self.assertFalse(self.verdict('error: ZordePrivateCompiledEntry not public\nparse error at line 1'))

    def test_missing_file_mixed_with_denial_is_refused(self):
        self.assertFalse(self.verdict('error: entries not visible\nno such file', 'private-storage-access'))

    def test_not_found_is_not_private(self):
        self.assertFalse(self.verdict('error: ZordePrivateCompiledEntry not found'))

    def test_storage_registry_name_alone_is_insufficient(self):
        self.assertFalse(self.verdict('error: ZordeScalarRegistry cannot access unrelated_field', 'private-storage-access'))

    def test_empty_is_refused(self):
        self.assertFalse(self.verdict(b''))

    def test_oversized_log_is_refused(self):
        self.assertFalse(self.verdict(b'error: ZordePrivateCompiledEntry not public\n' + b'X' * 65536))

    def test_non_utf8_is_refused(self):
        self.assertFalse(self.verdict(b'error: ZordePrivateCompiledEntry not public\xff'))

    def test_unknown_fixture_is_refused(self):
        self.assertFalse(self.verdict('error: ZordePrivateCompiledEntry is private', 'unknown-fixture'))

    def test_cli_accepts_valid_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            logfile = Path(tmp) / 'diagnostic.log'
            logfile.write_text('error: ZordePrivateCompiledEntry is private\n')
            self.assertEqual(verifier.main(['program', str(logfile), 'private-entry-import']), 0)

    def test_cli_denies_unsupported_fixture(self):
        self.assertEqual(verifier.main(['program', '/tmp/dummy', 'fake-fixture']), 2)


    def test_same_line_unrelated_private_entry_is_refused(self):
        self.assertFalse(self.verdict('error: ZordePrivateCompiledEntry unknown; Other is private'))

    def test_same_line_unrelated_private_storage_is_refused(self):
        self.assertFalse(self.verdict('error: entries unknown; Other is private', 'private-storage-access'))

    def test_same_line_unrelated_denial_prefix_is_refused(self):
        self.assertFalse(self.verdict('error: cannot access Other; ZordePrivateCompiledEntry unknown'))

    def test_adjacent_unrelated_denial_is_refused(self):
        self.assertFalse(self.verdict('error: Other is private\nZordePrivateCompiledEntry'))

    def test_unrelated_adjacent_storage_is_refused(self):
        self.assertFalse(self.verdict('error: Other field is private\nentries', 'private-storage-access'))

    def test_private_target_case_sensitive(self):
        self.assertFalse(self.verdict('error: ZordeprivateCompiledEntry is private'))

    def test_private_member_case_sensitive(self):
        self.assertFalse(self.verdict('error: ENTRIES is private', 'private-storage-access'))

    def test_quoted_member_denial_is_accepted(self):
        self.assertTrue(self.verdict('error: `entries` has private access', 'private-storage-access'))

    def test_isolated_multiline_wrong_symbol_is_refused(self):
        self.assertFalse(self.verdict('error: private type\nOtherClass\nZordePrivateCompiledEntry'))

    def test_malformed_nul_rejected(self):
        self.assertFalse(self.verdict(b'error: ZordePrivateCompiledEntry is private\x00'))

    def test_parse_failure_overrides_valid_denial(self):
        self.assertFalse(self.verdict('error: ZordePrivateCompiledEntry is private\nsyntax error'))

    def test_spurious_word_between_symbol_and_denial(self):
        self.assertFalse(self.verdict('error: entries variable is private', 'private-storage-access'))


if __name__ == '__main__':
    unittest.main()
