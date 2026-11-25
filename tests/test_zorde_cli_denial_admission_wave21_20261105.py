"""Real Zorde denial-check CLI boundaries, not compiler attestation."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).resolve().parents[1] / "scripts/check-zorde-denial.py"


class ZordeCliAdmissionWave21(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.file = Path(temporary.name) / "compiler.log"

    def run_checker(self, data, fixture="private-storage-access"):
        self.file.write_bytes(data)
        return subprocess.run([sys.executable, str(CHECKER), str(self.file), fixture],
                              capture_output=True, check=False).returncode

    def test_exact_private_storage_diagnostic(self):
        self.assertEqual(self.run_checker(b"error: cannot access entries\n"), 0)

    def test_exact_private_entry_import_diagnostic(self):
        self.assertEqual(self.run_checker(b"error: private class ZordePrivateCompiledEntry\n",
                                          "private-entry-import"), 0)

    def test_unknown_fixture_fails_as_usage_error(self):
        self.assertEqual(self.run_checker(b"entries is private", "other-fixture"), 2)

    def test_empty_file_is_not_denial_evidence(self):
        self.assertEqual(self.run_checker(b""), 1)

    def test_binary_nul_is_not_denial_evidence(self):
        self.assertEqual(self.run_checker(b"entries is private\x00"), 1)

    def test_invalid_utf8_is_not_denial_evidence(self):
        self.assertEqual(self.run_checker(b"entries is private\xff"), 1)

    def test_one_byte_over_log_budget_is_denied(self):
        self.assertEqual(self.run_checker(b"entries is private\n" + b"x" * 1_048_576), 1)

    def test_missing_log_path_is_not_denial_evidence(self):
        self.assertEqual(self.run_checker(b"entries is private"), 0)
        self.file.unlink()
        result = subprocess.run(
            [sys.executable, str(CHECKER), str(self.file), "private-storage-access"],
            capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
