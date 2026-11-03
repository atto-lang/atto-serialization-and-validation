#!/usr/bin/env python3
"""Offline receipt structure gate. Independent GitHub run/artifact verification remains mandatory."""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "fixtures/serde/adversarial-v1.json"
BACKENDS = (
    "java-graalvm",
    "llvm",
    "wasm",
    "cranelift-independent",
    "dotnet-nativeaot",
)


def digest(value: object, length: int, name: str) -> None:
    if not isinstance(value, str) or re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is None:
        raise ValueError(f"missing/invalid {name} (expected {length} lowercase hex digits)")


def positive(value: object, name: str) -> None:
    if type(value) is not int or value <= 0:
        raise ValueError(f"missing/nonpositive {name}")


def verify(receipt_dir: Path, head: str, tree: str) -> None:
    digest(head, 40, "expected head SHA")
    digest(tree, 40, "expected tree SHA")
    manifest_bytes = MANIFEST.read_bytes()
    manifest = json.loads(manifest_bytes)
    fixture_sha = hashlib.sha256(manifest_bytes).hexdigest()
    expected = {
        item["id"]: {
            "input_sha256": hashlib.sha256(bytes.fromhex(item["bytes_hex"])).hexdigest(),
            "expected_result": item["expected_result"],
            "expected_error_code": item.get("expected_error_code"),
            "expected_decoded": item.get("expected_decoded"),
        }
        for item in manifest["cases"]
    }
    if len(expected) != len(manifest["cases"]):
        raise ValueError("duplicate manifest fixture IDs")

    if not receipt_dir.is_dir() or receipt_dir.is_symlink():
        raise ValueError("receipt directory missing or is a symlink")
    entries = list(receipt_dir.iterdir())
    for entry in entries:
        if not entry.is_file() or entry.is_symlink():
            raise ValueError("receipt directory contains an unsafe entry")
        if entry.stat().st_size > 512 * 1024:
            raise ValueError("receipt JSON exceeds 512 KiB size limit")
    actual_names = sorted(entry.name for entry in entries)
    names = sorted(b + ".json" for b in BACKENDS)
    if actual_names != names:
        raise ValueError(f"expected only five receipts {names}, found {actual_names}")

    seen_job_ids = set()
    for backend in BACKENDS:
        evidence = json.loads((receipt_dir / (backend + ".json")).read_text())
        prefix = backend + ": "
        if evidence.get("backend") != backend or evidence.get("status") != "PASS":
            raise ValueError(prefix + "missing backend identity or PASS result")
        for key, want in (
            ("source_head_sha", head),
            ("source_tree_sha", tree),
            ("fixture_manifest_sha256", fixture_sha),
        ):
            if evidence.get(key) != want:
                raise ValueError(prefix + key + " mismatch")
        for key in (
            "compiler_sha256", "guest_artifact_sha256",
            "stdout_sha256", "stderr_sha256",
        ):
            digest(evidence.get(key), 64, prefix + key)
        for key in ("run_id", "job_id", "executed_step_count"):
            positive(evidence.get(key), prefix + key)
        if evidence["job_id"] in seen_job_ids:
            raise ValueError(prefix + "job ID reused by another backend")
        seen_job_ids.add(evidence["job_id"])
        for key in (
            "compiler_version", "target", "runner_os", "run_url", "job_url",
            "compile_command", "execute_command",
        ):
            if not isinstance(evidence.get(key), str) or not evidence[key].strip():
                raise ValueError(prefix + "missing " + key)
        # These are structural checks; an independent reviewer must still
        # query GitHub and verify run, job, actual step names and artifacts.
        run_url = evidence["run_url"]
        job_url = evidence["job_url"]
        parts = urlsplit(run_url)
        if (parts.scheme != "https" or parts.hostname != "github.com"
                or parts.username or parts.password or parts.query or parts.fragment):
            raise ValueError(prefix + "run URL is not a canonical GitHub Actions URL")
        match = re.fullmatch(
            r"/([a-zA-Z0-9_.-]+)/([a-zA-Z0-9_.-]+)/actions/runs/([0-9]+)",
            parts.path,
        )
        if not match or int(match.group(3)) != evidence["run_id"]:
            raise ValueError(prefix + "run URL/ID mismatch")
        if job_url != run_url + "/job/" + str(evidence["job_id"]):
            raise ValueError(prefix + "job URL/ID mismatch")

        artifact_kind = "wasm-module" if backend == "wasm" else "native-executable"
        if evidence.get("guest_artifact_kind") != artifact_kind:
            raise ValueError(prefix + "wrong native guest artifact kind")
        if type(evidence.get("guest_exit_code")) is not int or evidence["guest_exit_code"] != 0:
            raise ValueError(prefix + "missing/nonzero guest test exit code")
        if evidence.get("compiled_native_guest") is not True:
            raise ValueError(prefix + "native guest compiler execution missing")
        if evidence.get("executed_native_guest") is not True or evidence.get("host_only") is not False:
            raise ValueError(prefix + "guest run missing or host-only execution")
        if backend == "cranelift-independent" and evidence.get("independent_cranelift") is not True:
            raise ValueError(prefix + "independent Cranelift compiler not proven")
        if backend in ("java-graalvm", "llvm", "cranelift-independent", "dotnet-nativeaot"):
            if evidence.get("full_aot") is not True:
                raise ValueError(prefix + "native AOT evidence missing")
        mem = evidence.get("memory") or {}
        positive(mem.get("allocation_limit_bytes"), prefix + "allocation limit")
        peak = mem.get("peak_alloc_bytes")
        if type(peak) is not int or not 0 <= peak <= mem["allocation_limit_bytes"]:
            raise ValueError(prefix + "peak allocation outside limit")
        max_depth = mem.get("max_depth")
        if type(max_depth) is not int or not 0 <= max_depth <= 128:
            raise ValueError(prefix + "invalid parse depth limit")
        inst = evidence.get("instrumentation") or {}
        if inst.get("result") != "PASS" or not inst.get("kind"):
            raise ValueError(prefix + "sanitizer/equivalent instrumentation missing")
        digest(inst.get("evidence_sha256"), 64, prefix + "instrumentation digest")
        results = evidence.get("cases")
        if not isinstance(results, list) or len(results) != len(expected):
            raise ValueError(prefix + "incomplete cases")
        seen = set()
        for row in results:
            case_id = row.get("id")
            if case_id not in expected or case_id in seen:
                raise ValueError(prefix + "unknown/duplicate case ID " + str(case_id))
            seen.add(case_id)
            item = expected[case_id]
            if row.get("input_sha256") != item["input_sha256"]:
                raise ValueError(prefix + case_id + ": input hash mismatch")
            if row.get("result") != item["expected_result"]:
                raise ValueError(prefix + case_id + ": unexpected Ok/Err")
            if item["expected_result"] == "Err" and row.get("error_code") != item["expected_error_code"]:
                raise ValueError(prefix + case_id + ": error classification mismatch")
            if item["expected_result"] == "Ok":
                if row.get("error_code") not in (None, ""):
                    raise ValueError(prefix + case_id + ": Ok result contains error")
                if row.get("decoded_value") != item["expected_decoded"]:
                    raise ValueError(prefix + case_id + ": positive decoded value mismatch")
            elif row.get("decoded_value") is not None:
                raise ValueError(prefix + case_id + ": Err must not publish decoded value")
        print(backend + ": structurally complete guest receipt")
    print("STRUCTURAL_RECEIPT_GATE=PASS")
    print("INDEPENDENT_RUN_AND_ARTIFACT_VERIFICATION=STILL_REQUIRED")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--receipts", required=True, type=Path)
    p.add_argument("--head", required=True)
    p.add_argument("--tree", required=True)
    args = p.parse_args()
    try:
        verify(args.receipts, args.head, args.tree)
    except (OSError, KeyError, TypeError, ValueError) as exc:
        print("STRUCTURAL_RECEIPT_GATE=FAIL: " + str(exc), file=sys.stderr)
        sys.exit(1)
