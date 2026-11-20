"""Synthetic security regression for the source-local Zorde Contract IR runner.

Exercises the exact committed shell script in independent temporary Git trees;
no private source is copied, no TypeSpec compiler is executed, no tokens used.
"""
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile

# Assertions are part of these fail-closed tests, not optional diagnostics.
# -I ignores PYTHONOPTIMIZE; -S disables site hooks. Refuse nonisolated runs.
if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    raise SystemExit("ZORDE_CONTAINMENT_TEST_REFUSED: require python3 -I -S without -O")

SOURCE = Path(__file__).resolve().parents[2]
SCRIPT = (SOURCE / "scripts/test-zorde-contract-source-local.sh").read_bytes()
LAUNCHER = (SOURCE / "scripts/launch-zorde-contract-source-local.py").read_bytes()
WORK = Path(tempfile.mkdtemp(prefix="zorde-overlap-synthetic-", dir="/tmp"))

def git(root, *args):
    p = subprocess.run(["git", *args], cwd=root, capture_output=True,
                       text=True, timeout=10, check=False)
    assert p.returncode == 0, p.stderr
    return p.stdout.strip()

def scenario(label, expected, *, source_inside_validator=False,
             evidence_inside_validator=False, validator_inside_evidence=False,
             artifacts_symlink=False, artifacts_preexisting=None):
    folder = WORK / label
    folder.mkdir(mode=0o700)
    validator = folder / "public"
    root = validator / "private" if source_inside_validator else folder / "private"
    root.mkdir(parents=True)
    (root / "scripts").mkdir()
    (root / "scripts/test-zorde-contract-source-local.sh").write_bytes(SCRIPT)
    (root / "scripts/launch-zorde-contract-source-local.py").write_bytes(LAUNCHER)
    (root / ".gitignore").write_text("artifacts\n")
    git(root, "init", "-q")
    git(root, "add", ".")
    git(root, "-c", "user.name=Synthetic",
        "-c", "user.email=synthetic@example.invalid",
        "commit", "-qm", "reviewed")
    validator.mkdir(exist_ok=True)
    evidence = (validator / "secret-logs" if evidence_inside_validator
                else folder / "evidence")
    if validator_inside_evidence:
        validator = evidence / "public"
        validator.mkdir(parents=True)
    evidence.mkdir(mode=0o700, exist_ok=True)
    evidence.chmod(0o700)
    if artifacts_symlink:
        (root / "artifacts").symlink_to(folder / "off-checkout-output",
                                        target_is_directory=True)

    if artifacts_preexisting:
        output = root / "artifacts"
        if artifacts_preexisting == "file":
            output.write_text("must not overwrite this path")
        else:
            output.mkdir()
            if artifacts_preexisting == "dangling-child":
                (output / "zorde-scalars-parity.json").symlink_to(
                    folder / "outside-private-checkout.json")
            elif artifacts_preexisting == "nested":
                (output / "zorde-generated").mkdir()
                (output / "zorde-generated" / "keep").write_text("stale")
            elif artifacts_preexisting != "empty":
                raise ValueError(artifacts_preexisting)

    marker = folder / "bash-startup-executed"
    startup = folder / "malicious-bashenv.sh"
    startup.write_text("printf injected > " + str(marker) + "\n")
    env = os.environ.copy()
    env.update(
        BASH_ENV=str(startup), ENV=str(startup),
        EXPECTED_SOURCE_SHA=git(root, "rev-parse", "HEAD"),
        EXPECTED_SOURCE_TREE=git(root, "rev-parse", "HEAD^{tree}"),
        NODE_BIN=str(NODE),
        EXPECTED_NODE_SHA256=hashlib.sha256(NODE.read_bytes()).hexdigest(),
        TJSV_ROOT=str(validator),
        ZORDE_EVIDENCE_DIR=str(evidence),
    )
    # Positive control: prove this exact BASH_ENV payload executes in direct
    # noninteractive Bash. Otherwise "marker absent" would be a false pass.
    witness = subprocess.run(
        ["/usr/bin/bash", "--noprofile", "--norc", "-c", ":"],
        cwd=root, env=env, capture_output=True, text=True, timeout=15,
    )
    assert witness.returncode == 0, (label, witness.stderr)
    assert marker.is_file() and marker.read_bytes() == b"injected", (
        label, "malicious startup payload did not actually run")
    marker.unlink()

    proc = subprocess.run(
        [sys.executable, "-I", "-S",
         str(root / "scripts/launch-zorde-contract-source-local.py")],
        cwd=root, env=env, capture_output=True, text=True, timeout=15,
    )
    assert proc.returncode == 2, (label, proc.returncode, proc.stderr)
    assert expected in proc.stderr, (label, proc.stderr)
    assert not marker.exists(), (label, "inherited Bash startup code executed")
    assert not (evidence / "zorde-contract-static-receipt.json").exists(), label
    print("PASS", label, "refused:", expected)

try:
    NODE = WORK / "synthetic-node22"
    NODE.write_text("#!/bin/sh\nprintf 'v22.18.0\\n'\n")
    NODE.chmod(0o700)

    scenario("ignored-artifacts-symlink",
             "generated artifacts directory must not be a symlink",
             artifacts_symlink=True)
    scenario("stale-empty-artifacts",
             "generated artifacts path must be absent at start",
             artifacts_preexisting="empty")
    scenario("stale-dangling-artifact-child",
             "generated artifacts path must be absent at start",
             artifacts_preexisting="dangling-child")
    scenario("stale-nested-artifacts",
             "generated artifacts path must be absent at start",
             artifacts_preexisting="nested")
    scenario("artifacts-root-regular-file",
             "generated artifacts path is not a directory",
             artifacts_preexisting="file")
    scenario("evidence-inside-public",
             "private evidence must not reside inside public TJSV checkout",
             evidence_inside_validator=True)
    scenario("private-inside-public",
             "private source must not be nested in public TJSV checkout",
             source_inside_validator=True)
    scenario("public-inside-evidence",
             "public TJSV checkout must not be nested inside private evidence",
             validator_inside_evidence=True)
    scenario("disjoint-preflight", "TJSV checkout root mismatch")
    print("ZORDE_SOURCE_LOCAL_PATH_CONTAINMENT_SYNTHETIC_9_OF_9_PASSED")
finally:
    shutil.rmtree(WORK)
