#!/usr/bin/env python3
"""Synthetic, SOURCE/HOST-ONLY tests for five-backend receipt rejection rules."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "verify_serde_receipts", ROOT / "scripts/verify-serde-receipts.py"
)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)
MANIFEST = ROOT / "fixtures/serde/adversarial-v1.json"


class SyntheticSerdeReceiptGateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="serde-receipts-host-only-")
        self.addCleanup(self.tmp.cleanup)
        self.receipts = Path(self.tmp.name)
        self.head = "a" * 40
        self.tree = "b" * 40
        manifest_raw = MANIFEST.read_bytes()
        fixture_sha = hashlib.sha256(manifest_raw).hexdigest()
        rows = []
        for item in json.loads(manifest_raw)["cases"]:
            rows.append({
                "id": item["id"],
                "input_sha256": hashlib.sha256(bytes.fromhex(item["bytes_hex"])).hexdigest(),
                "result": item["expected_result"],
                "error_code": item.get("expected_error_code"),
            })
        self.data = {}
        for backend in gate.BACKENDS:
            self.data[backend] = {
                "backend": backend,
                "status": "PASS",
                "source_head_sha": self.head,
                "source_tree_sha": self.tree,
                "fixture_manifest_sha256": fixture_sha,
                "compiler_sha256": "c" * 64,
                "guest_artifact_sha256": "d" * 64,
                "stdout_sha256": "f" * 64,
                "stderr_sha256": "0" * 64,
                "guest_artifact_kind": "wasm-module" if backend == "wasm" else "native-executable",
                "guest_exit_code": 0,
                "compile_command": "SYNTHETIC-ONLY compile",
                "execute_command": "SYNTHETIC-ONLY run",
                "run_id": 1,
                "job_id": 2,
                "executed_step_count": 3,
                "compiler_version": "SYNTHETIC-NOT-A-REAL-COMPILER",
                "target": "synthetic",
                "runner_os": "synthetic",
                "run_url": "https://example.invalid/synthetic",
                "job_url": "https://example.invalid/synthetic/job",
                "compiled_native_guest": True,
                "executed_native_guest": True,
                "host_only": False,
                "independent_cranelift": backend == "cranelift-independent",
                "full_aot": backend != "wasm",
                "memory": {
                    "allocation_limit_bytes": 1024 * 1024,
                    "peak_alloc_bytes": 1024,
                    "max_depth": 128,
                },
                "instrumentation": {
                    "kind": "SYNTHETIC-ONLY",
                    "result": "PASS",
                    "evidence_sha256": "e" * 64,
                },
                "cases": copy.deepcopy(rows),
            }
        self.write_all()

    def write_all(self):
        for backend, content in self.data.items():
            (self.receipts / (backend + ".json")).write_text(json.dumps(content))

    def validate(self):
        with contextlib.redirect_stdout(io.StringIO()):
            gate.verify(self.receipts, self.head, self.tree)

    def test_structural_acceptance_is_not_real_guest_proof(self):
        # A synthetic receipt can satisfy structural checks; external API
        # provenance and actual guest execution are separate mandatory gates.
        self.validate()

    def test_fail_closed_on_tampered_evidence(self):
        candidates = [
            ("missing_steps", lambda e: e.update(executed_step_count=0)),
            ("not_compiled", lambda e: e.update(compiled_native_guest=False)),
            ("not_executed", lambda e: e.update(executed_native_guest=False)),
            ("host_only", lambda e: e.update(host_only=True)),
            ("head_mismatch", lambda e: e.update(source_head_sha="0" * 40)),
            ("tree_mismatch", lambda e: e.update(source_tree_sha="0" * 40)),
            ("fixture_hash", lambda e: e.update(fixture_manifest_sha256="0" * 64)),
            ("no_artifact_hash", lambda e: e.pop("guest_artifact_sha256")),
            ("missing_stdout_hash", lambda e: e.pop("stdout_sha256")),
            ("nonzero_guest_exit", lambda e: e.update(guest_exit_code=7)),
            ("wrong_artifact_kind", lambda e: e.update(guest_artifact_kind="mock")),
            ("bad_aot", lambda e: e.update(full_aot=False)),
            ("peak_above_limit", lambda e: e["memory"].update(peak_alloc_bytes=2**25)),
            ("no_sanitizer", lambda e: e["instrumentation"].update(result="NOT_RUN")),
            ("missing_case", lambda e: e["cases"].pop()),
            ("unexpected_ok", lambda e: e["cases"][0].update(result="Ok")),
            ("wrong_case_sha", lambda e: e["cases"][0].update(input_sha256="0" * 64)),
            ("wrong_error_code", lambda e: e["cases"][0].update(error_code="wrong")),
        ]
        for label, mutate in candidates:
            with self.subTest(label=label):
                baseline = copy.deepcopy(self.data)
                mutate(self.data["llvm"])
                self.write_all()
                with self.assertRaises((ValueError, KeyError)):
                    self.validate()
                self.data = baseline

    def test_independent_cranelift_cannot_be_host_substitute(self):
        self.data["cranelift-independent"]["independent_cranelift"] = False
        self.write_all()
        with self.assertRaises(ValueError):
            self.validate()

    def test_unrecognized_extra_receipt_is_rejected(self):
        (self.receipts / "unapproved-backend.json").write_text("{}")
        with self.assertRaises(ValueError):
            self.validate()


if __name__ == "__main__":
    unittest.main()
