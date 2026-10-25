#!/usr/bin/env bash
# Execute real Oreslang guest tests ONLY within the approved source trust boundary.
# Independent of GitHub Actions: an authorized ephemeral/self-hosted source-local
# runner may use it when hosted Actions cannot allocate any jobs.
set -euo pipefail
umask 077

die() { printf 'native Zorde admission refused: %s\n' "$*" >&2; exit 2; }
for cmd in git sha256sum timeout python3 grep; do
  command -v "$cmd" >/dev/null 2>&1 || die "required utility unavailable: $cmd"
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"

# Reject symlinked parent directories as well as a symlink at the leaf.
# Realpath alone resolves those components rather than proving the approved
# command path is unredirected. This is preflight hardening, not a substitute
# for runner isolation against same-UID path swaps during execution.
no_symlink_components() {
  python3 - "$1" <<'PY'
import os, stat, sys
path = sys.argv[1]
if not os.path.isabs(path):
    sys.exit(1)
current = "/"
for component in path.split("/"):
    if not component or component == ".":
        continue
    if component == "..":
        sys.exit(1)
    current = os.path.join(current, component)
    try:
        mode = os.lstat(current).st_mode
    except OSError:
        sys.exit(1)
    if stat.S_ISLNK(mode):
        sys.exit(1)
PY
}
[[ -n "${EXPECTED_SOURCE_SHA:-}" && "${EXPECTED_SOURCE_SHA}" =~ ^[0-9a-f]{40}$ ]] ||
  die "EXPECTED_SOURCE_SHA must be an explicit immutable 40-hex source commit"
[[ -n "${EXPECTED_SOURCE_TREE:-}" && "${EXPECTED_SOURCE_TREE}" =~ ^[0-9a-f]{40}$ ]] ||
  die "EXPECTED_SOURCE_TREE must be the reviewed root Git tree SHA"
[[ -n "${EXPECTED_COMPILER_SHA256:-}" && "${EXPECTED_COMPILER_SHA256}" =~ ^[0-9a-f]{64}$ ]] ||
  die "EXPECTED_COMPILER_SHA256 must be the independently approved compiler binary digest"
[[ -n "${ORESLANG_BIN:-}" ]] || die "ORESLANG_BIN must identify the pinned compiler executable"
[[ -n "${ZORDE_EVIDENCE_DIR:-}" ]] || die "ZORDE_EVIDENCE_DIR is required (outside checkout)"
[[ -d "$ZORDE_EVIDENCE_DIR" ]] || die "evidence directory must exist before test execution"
git_status="$(git status --porcelain --untracked-files=all)" ||
  die "unable to inspect source working tree"
[[ -z "$git_status" ]] ||
  die "source tree is not clean"
source_sha="$(git rev-parse HEAD)"
[[ "$source_sha" == "$EXPECTED_SOURCE_SHA" ]] ||
  die "checked-out head differs from approved source revision"
source_tree="$(git rev-parse 'HEAD^{tree}')"
[[ "$source_tree" == "$EXPECTED_SOURCE_TREE" ]] ||
  die "checked-out root Git tree differs from approved source snapshot"
source_root="$(git rev-parse --show-toplevel)"
[[ "$(cd "$source_root" && pwd -P)" == "$root" ]] ||
  die "source checkout root mismatch"
# Physical byte verification intentionally bypasses Git's index/status cache.
python3 "$root/scripts/verify-zorde-checkout.py" "$source_sha" "$source_tree" ||
  die "source bytes differ from reviewed Git tree"

case "$ORESLANG_BIN" in
  /*) ;;
  *) die "ORESLANG_BIN must be an absolute path (no PATH fallback)" ;;
esac
[[ ! -L "$ORESLANG_BIN" ]] ||
  die "compiler executable must not be a symlink"
no_symlink_components "$ORESLANG_BIN" ||
  die "compiler executable path must have no symlink components or traversal"
[[ -f "$ORESLANG_BIN" && -x "$ORESLANG_BIN" ]] ||
  die "approved compiler executable missing or not executable"
compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d ' ' -f 1)"
[[ "$compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] ||
  die "compiler executable digest differs from approved input"

[[ ! -L "$ZORDE_EVIDENCE_DIR" ]] ||
  die "evidence directory must not be a symlink"
no_symlink_components "$ZORDE_EVIDENCE_DIR" ||
  die "evidence directory path must have no symlink components or traversal"
out_dir="$(cd "$ZORDE_EVIDENCE_DIR" && pwd -P)"
case "$out_dir" in
  "$root"|"$root"/*) die "evidence must be outside the source checkout" ;;
esac

# Refuse to overwrite previously collected evidence. Require the evidence
# directory to be owned by this worker and not writable by other users.
# This is a source-local confidentiality/evidence-integrity precondition, NOT
# proof that the runner was independently approved by an administrator.
if ! python3 - "$out_dir" <<'PY'
import os, stat, sys
st = os.stat(sys.argv[1])
sys.exit(0 if (
    stat.S_ISDIR(st.st_mode) and st.st_uid == os.geteuid()
    and (stat.S_IMODE(st.st_mode) & 0o077) == 0
) else 1)
PY
then
  die "evidence directory must be caller-owned and inaccessible to group/world"
fi
for artifact in zorde-registry-native.log zorde-registry-native-receipt.json \
  zorde-public-export-control.log zorde-public-import-positive.log \
  zorde-private-entry-import-denial.log \
  zorde-private-storage-access-denial.log; do
  [[ ! -e "$out_dir/$artifact" && ! -L "$out_dir/$artifact" ]] ||
    die "evidence output already exists; use a fresh, private directory"
done

log="$out_dir/zorde-registry-native.log"
receipt="$out_dir/zorde-registry-native-receipt.json"
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
# Keep the parent's positive-import control as a separate compiler
# invocation. Its function signature must resolve the public class/import
# even if no guest stdout marker is emitted by this compile-only fixture.
positive_import_log="$out_dir/zorde-public-import-positive.log"
set +e
timeout --signal=TERM --kill-after=5s 60s \
  "$ORESLANG_BIN" "$root/fixtures/zorde-compile-positive/public-registry-import.ores" >"$positive_import_log" 2>&1
positive_import_status=$?
set -e
[[ "$positive_import_status" -eq 0 ]] ||
  die "public registry import positive control failed (exit=$positive_import_status)"

# Independently prove that public registry API execution emits the expected
# marker, before interpreting either private failure as visibility enforcement.
public_log="$out_dir/zorde-public-export-control.log"
set +e
timeout --signal=TERM --kill-after=5s 60s \
  "$ORESLANG_BIN" "$root/fixtures/zorde-compile-positive/public-export-control.ores" >"$public_log" 2>&1
public_status=$?
set -e
[[ "$public_status" -eq 0 ]] ||
  die "public registry export control failed (status=$public_status)"
grep -Fqx "native Zorde public export control passed" "$public_log" ||
  die "public registry export control success marker absent"

# A parse error is insufficient: negative fixtures must fail with a real
# export/visibility error from the same approved compiler that ran the suite.
for fixture in private-entry-import private-storage-access; do
  negative_log="$out_dir/zorde-${fixture}-denial.log"
  set +e
  timeout --signal=TERM --kill-after=5s 60s \
    "$ORESLANG_BIN" "$root/fixtures/zorde-compile-negative/${fixture}.ores" >"$negative_log" 2>&1
  negative_status=$?
  set -e
  [[ "$negative_status" -ne 0 ]] ||
    die "private registry fixture unexpectedly succeeded: ${fixture}"
  [[ "$negative_status" -ne 124 && "$negative_status" -ne 137 &&
     "$negative_status" -ne 126 && "$negative_status" -ne 127 &&
     "$negative_status" -ne 143 ]] ||
    die "private registry fixture timed out: ${fixture}"
  # Require both the current targeted visibility classifier and the stricter
  # same-line protected-symbol denial verifier. Unrelated/spliced diagnostics
  # cannot qualify the compiler-negative fixture.
  python3 "$root/scripts/zorde_visibility_diagnostic.py" "$negative_log" "$fixture" ||
    die "negative fixture lacked a targeted visibility diagnostic: ${fixture}"
  python3 "$root/scripts/check-zorde-denial.py" "$negative_log" "$fixture" ||
    die "negative fixture lacked a matching access-control diagnostic: ${fixture}"
done

# A failing Git command with empty stdout must not be interpreted as a clean
# checkout. Re-verify source identity, not just cleanliness: HEAD can move
# to a distinct commit with an identical tree without changing git status.
git_status="$(git status --porcelain --untracked-files=all)" ||
  die "unable to inspect source working tree after native validation"
[[ -z "$git_status" ]] ||
  die "native test modified source tree; cannot issue receipt"
post_source_sha="$(git rev-parse HEAD)" ||
  die "unable to inspect source HEAD after native validation"
[[ "$post_source_sha" == "$source_sha" ]] ||
  die "source HEAD changed during native validation"
post_source_tree="$(git rev-parse 'HEAD^{tree}')" ||
  die "unable to inspect source root tree after native validation"
[[ "$post_source_tree" == "$source_tree" ]] ||
  die "source root Git tree changed during native validation"
python3 "$root/scripts/verify-zorde-checkout.py" "$source_sha" "$source_tree" ||
  die "source bytes differ from reviewed Git tree after native validation"

# The compiler file must remain the reviewed executable throughout the run.
# This catches persistent mutation between entry checks and evidence issuance;
# a pinned launcher digest alone still does not attest its transitive runtime.
[[ ! -L "$ORESLANG_BIN" ]] ||
  die "compiler executable became a symlink during native validation"
no_symlink_components "$ORESLANG_BIN" ||
  die "compiler executable path acquired symlink components during native validation"
no_symlink_components "$ZORDE_EVIDENCE_DIR" ||
  die "evidence directory path acquired symlink components during native validation"
final_compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d ' ' -f 1)"
[[ "$final_compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] ||
  die "compiler executable changed during native validation"

# This is executable evidence *only for a separately approved compiler binary*.
# A launcher digest alone does not attest a transitive Java/JVM/LLVM dependency
# closure; that requires separately reviewed compiler build/release provenance.
SOURCE_SHA="$source_sha" SOURCE_TREE="$source_tree" COMPILER_SHA="$compiler_sha" NATIVE_LOG="$log" \
  NATIVE_RECEIPT="$receipt" NEGATIVE_DIR="$out_dir" python3 - <<'PY'
import hashlib, json, os
from pathlib import Path
log = Path(os.environ["NATIVE_LOG"]).read_bytes()
record = {
    "schema": "oreslang.zorde.source-local.native-execution/v1",
    "source_head_sha": os.environ["SOURCE_SHA"],
    "source_root_tree_sha": os.environ["SOURCE_TREE"],
    "compiler_executable_sha256": os.environ["COMPILER_SHA"],
    "native_log_sha256": hashlib.sha256(log).hexdigest(),
    "actual_native_guest_executed": True,
    "registry_marker_observed": True,
    "full_suite_marker_observed": True,
    "public_registry_import_control_passed": True,
    "public_registry_import_log_sha256": hashlib.sha256(
        (Path(os.environ["NEGATIVE_DIR"]) / "zorde-public-import-positive.log").read_bytes()
    ).hexdigest(),
    "public_export_control_executed": True,
    "public_export_control_log_sha256": hashlib.sha256(
        (Path(os.environ["NEGATIVE_DIR"]) / "zorde-public-export-control.log").read_bytes()
    ).hexdigest(),
    "process_exit_code": 0,
    "negative_private_entry_import_refused": True,
    "negative_private_storage_access_refused": True,
    "negative_diagnostic_sha256": {
        fixture: hashlib.sha256(
            (Path(os.environ["NEGATIVE_DIR"]) / f"zorde-{fixture}-denial.log").read_bytes()
        ).hexdigest()
        for fixture in ("private-entry-import", "private-storage-access")
    },
    "compiler_transitive_dependency_closure_attested": False,
    "graal_llvm_wasm_parity_certified": False,
    "type_ownership_cross_actor_certified": False,
    "release_ready": False,
}
# Exclusive creation prevents silently replacing a previous native receipt.
with Path(os.environ["NATIVE_RECEIPT"]).open("x", encoding="utf-8") as fp:
    fp.write(json.dumps(record, indent=2, sort_keys=True) + "\n")
PY
printf 'source-local Zorde guest test executed: receipt=%s\n' "$receipt"
