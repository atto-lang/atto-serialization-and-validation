#!/usr/bin/env bash
# Exact-source native Oreslang execution inside the approved source trust boundary.
# Does not authorize or perform a cross-organization Git export.
set -euo pipefail
die() { printf 'int64 native admission refused: %s\n' "$*" >&2; exit 2; }
for executable in git sha256sum timeout python3 grep; do
  command -v "$executable" >/dev/null 2>&1 ||
    die "required utility missing: $executable"
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"

# Hosted/private repository allocation is independent of the local trust gate.
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  [[ "${GITHUB_REPOSITORY:-}" == "ores-truffle-oreslang/oreslang-serialization-and-validation" ]] ||
    die "GitHub Actions runner belongs to the wrong repository"
fi

[[ "${EXPECTED_SOURCE_SHA:-}" =~ ^[0-9a-f]{40}$ ]] ||
  die "EXPECTED_SOURCE_SHA must be a reviewed immutable commit"
[[ "${EXPECTED_SOURCE_TREE:-}" =~ ^[0-9a-f]{40}$ ]] ||
  die "EXPECTED_SOURCE_TREE must be a reviewed immutable root tree"
[[ "${EXPECTED_COMPILER_SHA256:-}" =~ ^[0-9a-f]{64}$ ]] ||
  die "EXPECTED_COMPILER_SHA256 must name an independently approved compiler executable"
[[ -n "${ORESLANG_BIN:-}" ]] || die "ORESLANG_BIN must be provided"
[[ "${ORESLANG_BIN}" == /* ]] || die "ORESLANG_BIN must be an absolute executable path"
[[ -f "$ORESLANG_BIN" && -x "$ORESLANG_BIN" ]] || die "approved compiler executable unavailable"
[[ -n "${INT64_EVIDENCE_DIR:-}" ]] || die "INT64_EVIDENCE_DIR must be provided"
[[ -d "$INT64_EVIDENCE_DIR" ]] || die "INT64_EVIDENCE_DIR must already exist"

[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" ]] ||
  die "source head differs from reviewed input"
[[ "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] ||
  die "source tree differs from reviewed input"
[[ "$(cd "$(git rev-parse --show-toplevel)" && pwd -P)" == "$root" ]] ||
  die "unexpected checkout root"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
  die "source checkout is not clean"
actual_compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d ' ' -f 1)"
[[ "$actual_compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] ||
  die "compiler hash differs from reviewed input"

out_dir="$(cd "$INT64_EVIDENCE_DIR" && pwd -P)"
case "$out_dir" in
  "$root"|"$root"/*) die "evidence directory must be outside source checkout" ;;
esac

log="$out_dir/int64-codec-native.log"
receipt="$out_dir/int64-codec-native-receipt.json"
rm -f "$receipt"
set +e
timeout --signal=TERM --kill-after=5s 300s \
  env ORESLANG_BIN="$ORESLANG_BIN" bash "$root/scripts/test.sh" >"$log" 2>&1
status=$?
set -e
if [[ "$status" -ne 0 ]]; then
  printf 'native int64 suite did not pass: exit=%s; see %s\n' "$status" "$log" >&2
  exit "$status"
fi

grep -Fqx 'native exact signed-int64 codec tests passed' "$log" ||
  die "actual int64 assertions completion marker missing"
grep -Fqx 'serialization/validation native tests passed' "$log" ||
  die "complete native suite marker missing"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
  die "native compiler/test modified the exact source checkout"

EXPECTED_SOURCE_SHA="$EXPECTED_SOURCE_SHA" EXPECTED_SOURCE_TREE="$EXPECTED_SOURCE_TREE" \
  ACTUAL_COMPILER_SHA="$actual_compiler_sha" INT64_LOG="$log" INT64_RECEIPT="$receipt" \
  python3 - <<'PY'
import hashlib, json, os
from pathlib import Path
log_bytes = Path(os.environ["INT64_LOG"]).read_bytes()
receipt = {
    "schema": "oreslang.source-local.int64-decimal-codec/v1",
    "source_sha": os.environ["EXPECTED_SOURCE_SHA"],
    "source_tree": os.environ["EXPECTED_SOURCE_TREE"],
    "compiler_executable_sha256": os.environ["ACTUAL_COMPILER_SHA"],
    "native_stdout_stderr_sha256": hashlib.sha256(log_bytes).hexdigest(),
    "native_exit_code": 0,
    "int64_codec_guest_assertions_executed": True,
    "full_native_suite_marker_observed": True,
    "compiler_transitive_dependencies_attested": False,
    "native_graal_llvm_wasm_parity_verified": False,
    "release_ready": False,
}
Path(os.environ["INT64_RECEIPT"]).write_text(
    json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8"
)
PY
echo "source-local native int64 codec suite complete; receipt=$receipt"
