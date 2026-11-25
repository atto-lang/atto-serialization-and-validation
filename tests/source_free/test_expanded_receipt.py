import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from expanded_receipt import ARTIFACTS, FLAGS, NEGATIVE, verify_base


class BaseReceiptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.out = Path(self.tmp.name)
        self.head, self.tree, self.compiler = "1" * 40, "2" * 40, "3" * 64
        self.path = self.out / "zorde-registry-native-receipt.json"
        self.record = dict(schema="oreslang.zorde.source-local.native-execution/v1", source_head_sha=self.head, source_root_tree_sha=self.tree, compiler_executable_sha256=self.compiler, process_exit_code=0, release_ready=False, **dict.fromkeys(FLAGS, True))
        for key, name in ARTIFACTS.items():
            data = b"native Zorde compiled scalar registry tests passed\nserialization/validation native tests passed\nnative Zorde public export control passed\n"
            (self.out / name).write_bytes(data)
            self.record[key] = hashlib.sha256(data).hexdigest()
        self.record["negative_diagnostic_sha256"] = {}
        for fixture in NEGATIVE:
            data = b"synthetic negative log; no compiler executed\n"
            (self.out / ("zorde-" + fixture + "-denial.log")).write_bytes(data)
            self.record["negative_diagnostic_sha256"][fixture] = hashlib.sha256(data).hexdigest()
        self.save()

    def save(self):
        self.path.write_text(json.dumps(self.record), encoding="utf-8")

    def verify(self):
        return verify_base(self.out, self.head, self.tree, self.compiler)

    def test_structurally_complete_synthetic_control(self):
        self.assertEqual(self.verify(), hashlib.sha256(self.path.read_bytes()).hexdigest())

    def test_required_controls(self):
        for key in FLAGS:
            with self.subTest(key=key):
                del self.record[key]
                self.save()
                with self.assertRaises(ValueError):
                    self.verify()
                self.record[key] = True

    def test_duplicate_json(self):
        self.path.write_text(self.path.read_text()[:-1] + ',"process_exit_code":0}')
        with self.assertRaises(ValueError):
            self.verify()

    def test_boolean_status(self):
        self.record["process_exit_code"] = False
        self.save()
        with self.assertRaises(ValueError):
            self.verify()

    def test_wrong_identity(self):
        self.record["source_root_tree_sha"] = "f" * 40
        self.save()
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_marker_even_with_matching_hash(self):
        data = b"synthetic log missing markers\n"
        (self.out / ARTIFACTS["native_log_sha256"]).write_bytes(data)
        self.record["native_log_sha256"] = hashlib.sha256(data).hexdigest()
        self.save()
        with self.assertRaises(ValueError):
            self.verify()

    def test_changed_negative_log(self):
        (self.out / ("zorde-" + NEGATIVE[0] + "-denial.log")).write_bytes(b"changed")
        with self.assertRaises(ValueError):
            self.verify()

    def test_incomplete_negative_set(self):
        del self.record["negative_diagnostic_sha256"][NEGATIVE[0]]
        self.save()
        with self.assertRaises(ValueError):
            self.verify()

    def test_symlink_artifact(self):
        path = self.out / ARTIFACTS["native_log_sha256"]
        original = self.out / "original.log"
        path.rename(original)
        path.symlink_to(original)
        with self.assertRaises(ValueError):
            self.verify()

    def test_overclaim(self):
        self.record["release_ready"] = True
        self.save()
        with self.assertRaises(ValueError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
