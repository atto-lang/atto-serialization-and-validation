#!/usr/bin/env bash
# Execute real Oreslang guest tests ONLY within the approved source trust boundary.
# Independent of GitHub Actions: an authorized ephemeral/self-hosted source-local
# runner may use it when hosted Actions cannot allocate any jobs.
set -euo pipefail

die() { printf 'native Zorde admission refused: %s\n' "$*" >&2; exit 2; }
for cmd in git sha256sum timeout python3 grep; do
  command -v "$cmd" >/dev/null 2>&1 || die "required utility unavailable: $cmd"
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
[[ -n "${EXPECTED_SOURCE_SHA:-}" && "${EXPECTED_SOURCE_SHA}" =~ ^[0-9a-f]{40}$ ]] ||
  die "EXPECTED_SOURCE_SHA must be an explicit immutable 40-hex source commit"
[[ -n "${EXPECTED_COMPILER_SHA256:-}" && "${EXPECTED_COMPILER_SHA256}" =~ ^[0-9a-f]{64}$ ]] ||
  die "EXPECTED_COMPILER_SHA256 must be the independently approved compiler binary digest"
[[ -n "${ORESLANG_BIN:-}" ]] || die "ORESLANG_BIN must identify the pinned compiler executable"
[[ -n "${ZORDE_EVIDENCE_DIR:-}" ]] || die "ZORDE_EVIDENCE_DIR is required (outside checkout)"
[[ -d "$ZORDE_EVIDENCE_DIR" ]] || die "evidence directory must exist before test execution"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
  die "source tree is not clean"
source_sha="$(git rev-parse HEAD)"
[[ "$source_sha" == "$EXPECTED_SOURCE_SHA" ]] ||
  die "checked-out head differs from approved source revision"
source_root="$(git rev-parse --show-toplevel)"
[[ "$(cd "$source_root" && pwd -P)" == "$root" ]] ||
  die "source checkout root mismatch"

case "$ORESLANG_BIN" in
  /*) ;;
  *) die "ORESLANG_BIN must be an absolute path (no PATH fallback)" ;;
esac
[[ -f "$ORESLANG_BIN" && -x "$ORESLANG_BIN" ]] ||
  die "approved compiler executable missing or not executable"
compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d ' ' -f 1)"
[[ "$compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] ||
  die "compiler executable digest differs from approved input"

out_dir="$(cd "$ZORDE_EVIDENCE_DIR" && pwd -P)"
case "$out_dir" in
  "$root"|"$root"/*) die "evidence must be outside the source checkout" ;;
esac

log="$out_dir/zorde-registry-native.log"
receipt="$out_dir/zorde-registry-native-receipt.json"
rm -f "$log" "$receipt"
set +e
timeout --signal=TERM --kill-after=5s 300s \
  env ORESLANG_BIN="$ORESLANG_BIN" bash "$root/scripts/test.sh" >"$log" 2>&1
status=$?
set -e
if [[ "$status" -ne 0 ]]; then
  printf 'native test status=%s; diagnostic log retained at %s\n' "$status" "$log" >&2
  exit "$status"
fi
grep -Fqx "native Zorde compiled scalar registry tests passed" "$log" ||
  die "registry guest test success marker absent"
grep -Fqx "serialization/validation native tests passed" "$log" ||
  die "full native suite success marker absent"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
  die "native test modified source tree; cannot issue receipt"

# This is executable evidence *only for a separately approved compiler binary*.
# A launcher digest alone does not attest a transitive Java/JVM/LLVM dependency
# closure; that requires separately reviewed compiler build/release provenance.
SOURCE_SHA="$source_sha" COMPILER_SHA="$compiler_sha" NATIVE_LOG="$log" \
  NATIVE_RECEIPT="$receipt" python3 - <<'PY'
import hashlib, json, os
from pathlib import Path
log = Path(os.environ["NATIVE_LOG"]).read_bytes()
record = {
    "schema": "oreslang.zorde.source-local.native-execution/v1",
    "source_head_sha": os.environ["SOURCE_SHA"],
    "compiler_executable_sha256": os.environ["COMPILER_SHA"],
    "native_log_sha256": hashlib.sha256(log).hexdigest(),
    "actual_native_guest_executed": True,
    "registry_marker_observed": True,
    "full_suite_marker_observed": True,
    "process_exit_code": 0,
    "compiler_transitive_dependency_closure_attested": False,
    "graal_llvm_wasm_parity_certified": False,
    "type_ownership_cross_actor_certified": False,
    "release_ready": False,
}
Path(os.environ["NATIVE_RECEIPT"]).write_text(
    json.dumps(record, indent=2, sort_keys=True) + "\n",
    encoding="utf-8"
)
PY
printf 'source-local Zorde guest test executed: receipt=%s\n' "$receipt"
