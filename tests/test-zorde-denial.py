#!/usr/bin/env python3
"""Source-free evidence-verifier regression tests; NOT Oreslang guest execution."""
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest

path = pathlib.Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"
spec = importlib.util.spec_from_file_location("zorde_denial_check", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class CheckAccessDenial(unittest.TestCase):
    def assert_refused(self, text, fixture):
        self.assertFalse(module.valid_log(text.encode(), fixture))
    def assert_accepted(self, text, fixture):
        self.assertTrue(module.valid_log(text.encode(), fixture))
    def test_private_entry(self):
        self.assert_accepted("error: ZordePrivateCompiledEntry is private", "private-entry-import")
    def test_private_field(self):
        self.assert_accepted("error: cannot access member entries", "private-storage-access")
    def test_not_exported(self):
        self.assert_accepted("error: ZordePrivateCompiledEntry not exported", "private-entry-import")
    def test_unrelated_error(self):
        self.assert_refused("error: ZordePrivateCompiledEntry undefined identifier", "private-entry-import")
    def test_cross_line_entry_bypass(self):
        self.assert_refused("ZordePrivateCompiledEntry\nprivate class Unexpected", "private-entry-import")
    def test_cross_line_field_bypass(self):
        self.assert_refused("entries\nerror: symbol elsewhere is private", "private-storage-access")
    def test_unrelated_private(self):
        self.assert_refused("error: SomeOtherType is private", "private-entry-import")
    def test_registry_name_without_field(self):
        self.assert_refused("error: ZordeScalarRegistry is private", "private-storage-access")
    def test_partial_field_name(self):
        self.assert_refused("error: entriesNotAccessible is private", "private-storage-access")
    def test_unknown_fixture(self):
        self.assert_refused("error: ZordePrivateCompiledEntry is private", "unknown")
    def test_binary_log(self):
        self.assertFalse(module.valid_log(b"\x00error: ZordePrivateCompiledEntry is private", "private-entry-import"))
    def test_invalid_utf8(self):
        self.assertFalse(module.valid_log(b"\xfferror: entries is private", "private-storage-access"))
    def test_empty(self):
        self.assertFalse(module.valid_log(b"", "private-entry-import"))
    def test_unbounded_log(self):
        self.assertFalse(module.valid_log(b"error: entries is private\n" + b"a" * module.MAX_LOG_BYTES, "private-storage-access"))
    def test_very_long_line(self):
        self.assertFalse(module.valid_log(("error: entries is private " + "x" * 4096).encode(), "private-storage-access"))
    def test_command_line_accepts_valid(self):
        with tempfile.TemporaryDirectory() as d:
            f = pathlib.Path(d) / "denial.log"
            f.write_text("error: field entries is private\n", encoding="utf-8")
            result = subprocess.run([sys.executable, str(path), str(f), "private-storage-access"], check=False)
            self.assertEqual(result.returncode, 0)
    def test_command_line_rejects_spliced_lines(self):
        with tempfile.TemporaryDirectory() as d:
            f = pathlib.Path(d) / "denial.log"
            f.write_text("entries\nerror: unrelated class is private\n", encoding="utf-8")
            result = subprocess.run([sys.executable, str(path), str(f), "private-storage-access"], check=False)
            self.assertEqual(result.returncode, 1)

    def test_same_line_unrelated_private_entry(self):
        self.assert_refused("error: ZordePrivateCompiledEntry unknown; Other is private", "private-entry-import")

    def test_same_line_unrelated_private_member(self):
        self.assert_refused("error: entries unknown; Other is private", "private-storage-access")

    def test_same_line_denial_precedes_unrelated_target(self):
        self.assert_refused("error: Other is private; ZordePrivateCompiledEntry unknown", "private-entry-import")

    def test_same_line_denial_follows_unrelated_target(self):
        self.assert_refused("error: entries unexpected; cannot access Other", "private-storage-access")

    def test_same_line_access_denied_other(self):
        self.assert_refused("error: ZordePrivateCompiledEntry not found; access denied to Other", "private-entry-import")

    def test_same_line_spurious_intermediate_words(self):
        self.assert_refused("error: entries variable is private", "private-storage-access")

    def test_explicit_private_declaration(self):
        self.assert_accepted("error: private class ZordePrivateCompiledEntry", "private-entry-import")

    def test_private_access_with_quotes(self):
        self.assert_accepted("error: `entries` has private access", "private-storage-access")

    def test_import_denial(self):
        self.assert_accepted("error: cannot import ZordePrivateCompiledEntry", "private-entry-import")

if __name__ == "__main__":
    unittest.main(verbosity=2)
