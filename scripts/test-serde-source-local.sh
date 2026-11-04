#!/usr/bin/env bash
# Run real guest integration tests only inside an independently approved,
# source-local, ephemeral runner. No cross-org mirroring or AOT claim.
set -euo pipefail
die() { printf 'serde guest admission refused: %s\n' "$*" >&2; exit 2; }

for bin in git python3 sha256sum timeout mktemp grep; do
  command -v "$bin" >/dev/null 2>&1 || die "missing required tool: $bin"
done
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  [[ "${GITHUB_REPOSITORY:-}" == "ores-truffle-oreslang/oreslang-serialization-and-validation" ]] ||
    die "runner is not in the approved source repository"
fi

[[ "${EXPECTED_SOURCE_SHA:-}" =~ ^[0-9a-f]{40}$ ]] || die "missing immutable approved HEAD"
[[ "${EXPECTED_SOURCE_TREE:-}" =~ ^[0-9a-f]{40}$ ]] || die "missing immutable approved tree"
[[ "${EXPECTED_COMPILER_SHA256:-}" =~ ^[0-9a-f]{64}$ ]] ||
  die "missing independently approved compiler digest"
[[ "${ORESLANG_BIN:-}" = /* ]] || die "approved compiler path must be absolute"
[[ -f "$ORESLANG_BIN" && -x "$ORESLANG_BIN" && ! -L "$ORESLANG_BIN" ]] ||
  die "approved compiler path is not an ordinary executable"
[[ "${SERDE_EVIDENCE_ROOT:-}" = /* ]] || die "approved evidence root must be absolute"
[[ -d "$SERDE_EVIDENCE_ROOT" && ! -L "$SERDE_EVIDENCE_ROOT" ]] ||
  die "approved evidence root is not an ordinary directory"
case "$(cd "$SERDE_EVIDENCE_ROOT" && pwd -P)" in
  "$root"|"$root"/*) die "evidence must remain outside source checkout" ;;
esac
case "$(cd "$(dirname "$ORESLANG_BIN")" && pwd -P)/$(basename "$ORESLANG_BIN")" in
  "$root"/*) die "compiler must not originate in the source checkout" ;;
esac
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  [[ "$GITHUB_SHA" == "$EXPECTED_SOURCE_SHA" ]] || die "approved job SHA differs"
fi

actual_compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d ' ' -f 1)"
[[ "$actual_compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] ||
  die "compiler digest mismatch"

# Fail closed on tracked/untracked/ignored files, filters, symlinks, index
# mismatch and incorrect executable modes; check again AFTER running.
source_integrity() {
  python3 -B scripts/verify-serde-worktree.py \
    --root "$root" \
    --head "$EXPECTED_SOURCE_SHA" \
    --tree "$EXPECTED_SOURCE_TREE"
}
source_integrity

# Source/host-level contract tests MUST pass before running the guest.
python3 -B scripts/check-serde-fixtures.py
python3 -B -m unittest discover -s tests/source_free -p 'test_serde_*_gate.py'
source_integrity

umask 077
evidence_dir="$(mktemp -d "$SERDE_EVIDENCE_ROOT/serde-guest.XXXXXXXX")" ||
  die "cannot allocate private evidence directory"
log="$evidence_dir/guest-stdout-stderr.log"
receipt="$evidence_dir/source-local-receipt.json"

set +e
timeout --signal=TERM --kill-after=5s 300s \
  env ORESLANG_BIN="$ORESLANG_BIN" bash "$root/scripts/test.sh" >"$log" 2>&1
status=$?
set -e
if [[ "$status" -ne 0 ]]; then
  printf 'serde guest failed with exit=%d; private log retained\n' "$status" >&2
  exit "$status"
fi
grep -Fqx 'serialization/validation native tests passed' "$log" ||
  die "real guest completion marker is missing"
source_integrity

SERDE_LOG="$log" SERDE_RECEIPT="$receipt" \
SERDE_MANIFEST="$root/fixtures/serde/adversarial-v1.json" \
SERDE_COMPILER_SHA="$actual_compiler_sha" \
  python3 -B - <<'PY'
import hashlib
import json
import os
from pathlib import Path

def sha256(path: str) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

receipt = {
    "schema": "atto.serde.source-local-guest/v1",
    "source_head_sha": os.environ["EXPECTED_SOURCE_SHA"],
    "source_tree_sha": os.environ["EXPECTED_SOURCE_TREE"],
    "fixture_manifest_sha256": sha256(os.environ["SERDE_MANIFEST"]),
    "compiler_sha256": os.environ["SERDE_COMPILER_SHA"],
    "guest_log_sha256": sha256(os.environ["SERDE_LOG"]),
    "guest_exit_code": 0,
    "guest_suite_completion_marker": True,
    "backend": "UNVERIFIED",
    "full_aot": False,
    "asan_ubsan_verified": False,
    "five_backend_parity": False,
    "artifact_promotion_allowed": False,
}
Path(os.environ["SERDE_RECEIPT"]).write_text(
    json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8"
)
PY
sha256sum "$receipt"
echo "SERDE_SOURCE_LOCAL_GUEST_EXECUTED=PASS (NOT AOT OR FIVE-BACKEND PARITY)"
echo "SERDE_EVIDENCE_RETAINED_IN_APPROVED_PRIVATE_DIRECTORY"
