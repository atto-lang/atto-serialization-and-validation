#!/usr/bin/env python3
"""Host-only negative tests for authored serde source/fixture consistency."""
import contextlib
import importlib.util
import io
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "serde_source_fixture_checker", ROOT / "scripts/check-serde-fixtures.py"
)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class SerdeSourceFixtureGateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="serde-fixture-source-only-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.paths = (
            "fixtures/serde/adversarial-v1.json",
            "tests/main.ores",
            "tests/json_test.ores",
            "tests/messagepack_test.ores",
        )
        for name in self.paths:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        self.original_root = checker.ROOT
        self.original_manifest = checker.MANIFEST
        checker.ROOT = self.root
        checker.MANIFEST = self.root / "fixtures/serde/adversarial-v1.json"
        self.addCleanup(self.restore_checker)

    def restore_checker(self):
        checker.ROOT = self.original_root
        checker.MANIFEST = self.original_manifest

    def verify(self):
        with contextlib.redirect_stdout(io.StringIO()):
            checker.check()

    def rewrite(self, file, before, after):
        path = self.root / file
        content = path.read_text()
        if before not in content:
            raise AssertionError("test tamper anchor missing: " + before)
        path.write_text(content.replace(before, after, 1))

    def test_pristine_source_vectors_and_typed_codes(self):
        # Source-only PASS, not native guest execution.
        self.verify()

    def test_manifest_byte_tampering_rejected(self):
        path = self.root / "fixtures/serde/adversarial-v1.json"
        obj = json.loads(path.read_text())
        obj["cases"][0]["bytes_hex"] = "00"
        path.write_text(json.dumps(obj))
        with self.assertRaises(ValueError):
            self.verify()

    def test_json_typed_error_code_tampering_rejected(self):
        self.rewrite(
            "tests/json_test.ores",
            'leading_zero_detail.error.code == "invalid_number"',
            'leading_zero_detail.error.code == "wrong_code"',
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_messagepack_error_code_with_numeric_suffix_rejected(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            'new List<int>(217, 2, 192, 175), "invalid_utf8"',
            'new List<int>(217, 2, 192, 175), "wrong_code"',
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_json_typed_bytes_tampering_rejected(self):
        self.rewrite(
            "tests/json_test.ores",
            "val DecodeAttempt leading_zero_detail = decode_attempt(\n    new List<int>(48, 49)",
            "val DecodeAttempt leading_zero_detail = decode_attempt(\n    new List<int>(48, 50)",
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_guest_main_call_rejected(self):
        self.rewrite("tests/main.ores", "  run_messagepack_tests();", "  // removed call")
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_positive_assertion_rejected(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            "require_true(!array16_exact.is_err()",
            "require_true(array16_exact.is_err()",
        )
        with self.assertRaises(ValueError):
            self.verify()


    def test_commented_out_guest_call_is_not_admitted(self):
        self.rewrite(
            "tests/main.ores",
            "  run_messagepack_tests();",
            "  // run_messagepack_tests();",
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_commented_out_result_assertion_is_not_admitted(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            "  require_true(!array16_exact.is_err()",
            "  // require_true(!array16_exact.is_err()",
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_commented_out_typed_diagnostic_is_not_admitted(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            '  require_messagepack_error(new List<int>(217, 2, 192, 175), "invalid_utf8");',
            '  // require_messagepack_error(new List<int>(217, 2, 192, 175), "invalid_utf8");',
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_unterminated_block_comment_is_rejected(self):
        path = self.root / "tests/messagepack_test.ores"
        path.write_text(path.read_text() + "\n/* unterminated")
        with self.assertRaises(ValueError):
            self.verify()

    def test_comment_tokens_inside_string_literals_are_preserved(self):
        self.rewrite(
            "tests/json_test.ores",
            '"JSON second root value must reject"',
            '"JSON // second /* root */ value must reject"',
        )
        self.verify()


    def test_wrong_positive_value_in_guest_is_rejected(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            "array16_decoded.value.array_value[1].number_lexeme[0], 50",
            "array16_decoded.value.array_value[1].number_lexeme[0], 51",
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_commented_positive_value_assertion_is_rejected(self):
        self.rewrite(
            "tests/messagepack_test.ores",
            "  require_int(map16_decoded.value.map_value[0].value.number_lexeme[0], 50",
            "  // require_int(map16_decoded.value.map_value[0].value.number_lexeme[0], 50",
        )
        with self.assertRaises(ValueError):
            self.verify()

    def test_positive_manifest_value_tampering_is_rejected(self):
        path = self.root / "fixtures/serde/adversarial-v1.json"
        data = json.loads(path.read_text())
        for case in data["cases"]:
            if case["id"] == "messagepack.array16_exact":
                case["expected_decoded"]["integer_elements"] = [1, 3]
                break
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            self.verify()



if __name__ == "__main__":
    unittest.main()
