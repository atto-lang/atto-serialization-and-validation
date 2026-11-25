#!/usr/bin/env python3
"""Additional native probes; run only after the independently approved base suite."""
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import stat
import subprocess
import sys

from expanded_visibility import accepted, TARGETS, CONSTRUCTORS, MAX_LOG

ROOT = Path(__file__).resolve().parents[1]
POSITIVES = {
    "fixtures/json-frame-visibility/public-control.ores": "json frame public control passed",
    "fixtures/sealed-schema/public-control.ores": "sealed schema public control passed",
    "tests/json_cursor_native_smoke.ores": "native Zorde JSON cursor ownership passed",
    "tests/json_cursor_borrow_smoke.ores": "native Zorde JSON cursor smoke passed",
}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def safe_path(path):
    require(path.is_absolute() and ".." not in path.parts, "invalid absolute path")
    for part in [path, *path.parents]:
        require(not part.is_symlink(), "symlink path forbidden")
    return path


def digest(path):
    require(path.is_file() and not path.is_symlink(), "not a regular evidence file")
    with path.open("rb") as stream:
        sha = hashlib.sha256()
        for block in iter(lambda: stream.read(65536), b""):
            sha.update(block)
        return sha.hexdigest()


def verify_source(sha, tree, compiler, compiler_sha):
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip() == sha, "source HEAD changed")
    require(subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], cwd=ROOT).decode().strip() == tree, "source tree changed")
    subprocess.run([sys.executable, "-B", str(ROOT / "scripts/verify-zorde-checkout.py"), sha, tree], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    require(os.access(safe_path(compiler), os.X_OK) and digest(compiler) == compiler_sha, "compiler identity mismatch")


def limit_log():
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_LOG, MAX_LOG))


def main():
    sha = os.environ["EXPECTED_SOURCE_SHA"]
    tree = os.environ["EXPECTED_SOURCE_TREE"]
    compiler_sha = os.environ["EXPECTED_COMPILER_SHA256"]
    require(re.fullmatch(r"[a-f0-9]{40}", sha) and re.fullmatch(r"[a-f0-9]{40}", tree) and re.fullmatch(r"[a-f0-9]{64}", compiler_sha), "invalid approved digests")
    compiler = safe_path(Path(os.environ["ORESLANG_BIN"]))
    out = safe_path(Path(os.environ["ZORDE_EVIDENCE_DIR"]))
    require(out.is_dir() and ROOT != out and ROOT not in out.parents, "evidence directory must be outside checkout")
    st = out.stat()
    require(st.st_uid == os.geteuid() and stat.S_IMODE(st.st_mode) & 0o077 == 0, "unsafe evidence permissions")
    base = out / "zorde-registry-native-receipt.json"
    require(base.is_file() and not base.is_symlink() and base.stat().st_size <= MAX_LOG, "missing or oversized base receipt")
    record = json.loads(base.read_bytes())
    require(record.get("source_head_sha") == sha and record.get("source_root_tree_sha") == tree and record.get("compiler_executable_sha256") == compiler_sha and record.get("actual_native_guest_executed") is True and record.get("process_exit_code") == 0, "base native receipt does not match approved inputs")
    require(digest(out / "zorde-registry-native.log") == record.get("native_log_sha256"), "base native log mismatch")
    base_sha = digest(base)
    receipt = out / "zorde-expanded-native-receipt.json"
    require(not receipt.exists() and not receipt.is_symlink(), "expanded receipt already exists")
    verify_source(sha, tree, compiler, compiler_sha)
    logs = {}

    def run(path, label):
        log = out / ("expanded-" + label + ".log")
        with log.open("xb") as output:
            result = subprocess.run(["timeout", "--signal=TERM", "--kill-after=5s", "60s", str(compiler), str(ROOT / path)], cwd=ROOT, stdout=output, stderr=subprocess.STDOUT, preexec_fn=limit_log)
        require(log.stat().st_size < MAX_LOG, "oversized probe log")
        content = log.read_bytes()
        logs[label] = {"exit": result.returncode, "log_sha256": hashlib.sha256(content).hexdigest()}
        return result.returncode, content

    for path, marker in POSITIVES.items():
        label = path.replace("/", "-").removesuffix(".ores")
        status, log = run(path, label)
        require(status == 0 and marker.encode() in log.splitlines(), "positive native control failed: " + label)
    for fixture in [*TARGETS, *CONSTRUCTORS]:
        status, log = run("fixtures/" + fixture + ".ores", fixture.replace("/", "-"))
        require(1 <= status <= 123 and accepted(log, fixture), "targeted negative diagnostic absent: " + fixture)
    verify_source(sha, tree, compiler, compiler_sha)
    require(digest(base) == base_sha, "base receipt changed")
    with receipt.open("x", encoding="utf-8") as fp:
        json.dump({"schema": "atto.zorde.expanded-native-probes/v1", "source_head_sha": sha, "source_root_tree_sha": tree, "compiler_executable_sha256": compiler_sha, "base_native_receipt_sha256": base_sha, "probes": logs, "release_ready": False, "cross_actor_ownership_certified": False}, fp, indent=2, sort_keys=True)
        fp.write("\n")
    print("expanded native probes executed; evidence retained privately")


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as exc:
        print("expanded native probe refusal:", type(exc).__name__, file=sys.stderr)
        sys.exit(2)
