#!/usr/bin/env bash
# Run the real generated guest exclusively on an approved source-local machine.
# Never export/mirror a private source tree to obtain an alternate runner.
set -euo pipefail
umask 077
deny(){ printf 'ZORDE_GENERATED_GUEST_REFUSED: %s\n' "$*" >&2; exit 2; }
for utility in git sha256sum timeout python3 grep; do
  command -v "$utility" >/dev/null 2>&1 || deny "utility unavailable"
done
for variable in EXPECTED_SOURCE_SHA EXPECTED_SOURCE_TREE EXPECTED_COMPILER_SHA256 \
  EXPECTED_CONTRACT_IR_SHA256 EXPECTED_PARITY_REPORT_SHA256 \
  EXPECTED_VERIFICATION_SHA256 ORESLANG_BIN ZORDE_EVIDENCE_DIR; do
  [[ -n "$(printenv "$variable" || true)" ]] || deny "required reviewed environment input missing"
done

root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$root"
[[ "$EXPECTED_SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]] || deny "invalid expected source SHA"
[[ "$EXPECTED_SOURCE_TREE" =~ ^[0-9a-f]{40}$ ]] || deny "invalid expected root tree"
[[ "$EXPECTED_COMPILER_SHA256" =~ ^[0-9a-f]{64}$ ]] || deny "invalid expected compiler digest"
for reviewed in "$EXPECTED_CONTRACT_IR_SHA256" "$EXPECTED_PARITY_REPORT_SHA256" "$EXPECTED_VERIFICATION_SHA256"; do
  [[ "$reviewed" =~ ^[0-9a-f]{64}$ ]] || deny "invalid externally reviewed evidence digest"
done
[[ "$ORESLANG_BIN" = /* && -f "$ORESLANG_BIN" && -x "$ORESLANG_BIN" && ! -L "$ORESLANG_BIN" ]] ||
  deny "compiler must be an absolute real executable, not a symlink"
[[ -d "$ZORDE_EVIDENCE_DIR" && ! -L "$ZORDE_EVIDENCE_DIR" ]] ||
  deny "evidence directory must preexist and must not be a symlink"
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" ]] || deny "source SHA changed"
[[ "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] || deny "source tree changed"
[[ "$(git rev-parse --show-toplevel)" == "$root" ]] || deny "unexpected repository root"
git diff --quiet && git diff --cached --quiet || deny "tracked tree is dirty"
while IFS= read -r -d '' name; do
  [[ "$name" == artifacts/* ]] || deny "untracked files outside generated artifacts"
done < <(git ls-files --others --exclude-standard -z)

# The three evidence hashes must be obtained from a separately reviewed
# source-local parity/verification run. A SHA the untrusted artifact supplier
# computed for its own forged files does NOT authenticate those artifacts.
[[ "$(sha256sum artifacts/zorde-scalars-contract-ir.json | cut -d' ' -f1)" == "$EXPECTED_CONTRACT_IR_SHA256" ]] ||
  deny "Contract IR differs from independently reviewed bytes"
[[ "$(sha256sum artifacts/zorde-scalars-parity.json | cut -d' ' -f1)" == "$EXPECTED_PARITY_REPORT_SHA256" ]] ||
  deny "parity receipt differs from independently reviewed bytes"
[[ "$(sha256sum artifacts/zorde-scalars-consumer-verification.json | cut -d' ' -f1)" == "$EXPECTED_VERIFICATION_SHA256" ]] ||
  deny "consumer verification differs from independently reviewed bytes"
compiler_sha="$(sha256sum "$ORESLANG_BIN" | cut -d' ' -f1)"
[[ "$compiler_sha" == "$EXPECTED_COMPILER_SHA256" ]] || deny "compiler digest changed"
out="$(cd "$ZORDE_EVIDENCE_DIR" && pwd -P)"
case "$out" in
  "$root"|"$root"/*) deny "evidence must be outside source checkout";;
esac
python3 - "$out" <<'PY' || deny "evidence directory policy check failed"
import os,stat,sys
s=os.lstat(sys.argv[1])
assert stat.S_ISDIR(s.st_mode) and s.st_uid==os.geteuid()
assert stat.S_IMODE(s.st_mode)&0o077==0
PY
log="$out/zorde-generated-smoke-native.log"
receipt="$out/zorde-generated-smoke-native-receipt.json"
before="$out/zorde-generated-inputs-before.json"
[[ ! -e "$log" && ! -L "$log" && ! -e "$receipt" && ! -L "$receipt" &&
   ! -e "$before" && ! -L "$before" ]] ||
  deny "use a fresh private evidence directory"

python3 - "$root" <<'PY' || deny "source-bound TJSV/IR/guest receipt missing or inconsistent"
import hashlib,json,stat,sys
from pathlib import Path
root=Path(sys.argv[1])
for directory in ('artifacts','artifacts/zorde-generated'):
    s=(root/directory).lstat()
    assert stat.S_ISDIR(s.st_mode),directory
def safe(p):
    p=root/p
    s=p.lstat()
    assert stat.S_ISREG(s.st_mode) and s.st_nlink==1,p
    return p.read_bytes()
def obj(p): return json.loads(safe(p))
def sha(p): return hashlib.sha256(safe(p)).hexdigest()
v=obj('artifacts/zorde-scalars-consumer-verification.json')
ir=obj('artifacts/zorde-scalars-contract-ir.json')
plan=obj('artifacts/zorde-generated/registry-plan.json')
built=obj('artifacts/zorde-generated/build-complete.json')
assert v['schema']=='ores.typespec-json-schema-validator.consumer-verification-receipt/v1'
assert v['status']=='passed' and v['admissible'] is True
assert v['expectedIrId']==v['computedIrId']==v['suppliedIrId']==ir['irId']
assert v['declarationIds']==['Zorde.Validation.Port','Zorde.Validation.ShortLabel']
assert ir['status']=='passed' and ir['admissible'] is True
assert plan['irId']==ir['irId']
assert [x['schemaId'] for x in plan['entries']]==['Zorde.Validation.Port','Zorde.Validation.ShortLabel']
assert plan['releaseReady'] is False and plan['runtimeImplementationVerified'] is False
assert built['schema']=='oreslang.zorde.static-registry-build-complete/v1'
assert built['irId']==ir['irId'] and built['tjsvRunId']==plan['tjsvRunId']
assert built['guestSmokeCompiled'] is False and built['guestSmokeExecuted'] is False
assert built['nativeGuestExecuted'] is False and built['releaseReady'] is False
assert built['planSha256']==sha('artifacts/zorde-generated/registry-plan.json')
assert built['guestSha256']==sha('artifacts/zorde-generated/registry-specs.ores')
assert built['smokeSha256']==sha('artifacts/zorde-generated/registry-smoke.ores')
PY

# Capture an exclusive pre-execution manifest of all seven authority and
# derived guest inputs; evidence emitted after execution must match precisely.
# This closes the ordinary precheck-to-receipt mutation window. It cannot
# attest a compromised runtime, so the compiler dependency closure stays
# explicitly uncertified in the native result.
ROOT="$root" INPUT_MANIFEST="$before" python3 - <<'PY' ||
  deny "unable to create independently digest-bound guest input manifest"
import hashlib,json,os,stat
from pathlib import Path
root=Path(os.environ["ROOT"])
paths=[
  "contracts/zorde/scalars.tsp",
  "contracts/zorde/scalars.schema.json",
  "artifacts/zorde-scalars-parity.json",
  "artifacts/zorde-scalars-contract-ir.json",
  "artifacts/zorde-scalars-consumer-verification.json",
  "artifacts/zorde-generated/build-complete.json",
  "artifacts/zorde-generated/registry-plan.json",
  "artifacts/zorde-generated/registry-specs.ores",
  "artifacts/zorde-generated/registry-smoke.ores",
]
def attested(path):
    p=root/path
    st=p.lstat()
    assert stat.S_ISREG(st.st_mode) and st.st_nlink==1,path
    return hashlib.sha256(p.read_bytes()).hexdigest()
manifest={"schema":"oreslang.zorde.native-guest-inputs/v1",
          "files":{name:attested(name) for name in paths}}
with Path(os.environ["INPUT_MANIFEST"]).open("x",encoding="utf8") as f:
    json.dump(manifest,f,sort_keys=True,indent=2)
    f.write("\n")
PY

before_sha="$(sha256sum "$before" | cut -d' ' -f1)"
[[ "$before_sha" =~ ^[0-9a-f]{64}$ ]] || deny "invalid pre-execution manifest digest"
set +e
timeout --signal=TERM --kill-after=5s 180s "$ORESLANG_BIN" \
  "$root/artifacts/zorde-generated/registry-smoke.ores" >"$log" 2>&1
status=$?
set -e
[[ "$status" -eq 0 ]] || deny "guest compiler/execution failed, see native log"
grep -Fqx 'zorde generated registry guest smoke passed' "$log" ||
  deny "actual guest success marker absent"

[[ "$(sha256sum "$before" | cut -d' ' -f1)" == "$before_sha" ]] ||
  deny "pre-execution manifest was modified during guest execution"
ROOT="$root" INPUT_MANIFEST="$before" python3 - <<'PY' ||
  deny "source authority or generated guest changed during execution"
import hashlib,json,os,stat
from pathlib import Path
root=Path(os.environ["ROOT"])
manifest=json.loads(Path(os.environ["INPUT_MANIFEST"]).read_text(encoding="utf8"))
assert manifest["schema"]=="oreslang.zorde.native-guest-inputs/v1"
assert len(manifest["files"])==9
for name,expected in manifest["files"].items():
    p=root/name
    st=p.lstat()
    assert stat.S_ISREG(st.st_mode) and st.st_nlink==1,name
    assert hashlib.sha256(p.read_bytes()).hexdigest()==expected,name
PY

[[ "$(sha256sum "$ORESLANG_BIN" | cut -d' ' -f1)" == "$EXPECTED_COMPILER_SHA256" ]] ||
  deny "compiler executable changed during test"
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" &&
   "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] ||
  deny "source revision changed during test"
git diff --quiet && git diff --cached --quiet || deny "tracked checkout changed after test"
while IFS= read -r -d '' name; do
  [[ "$name" == artifacts/* ]] || deny "guest execution created unexpected untracked source files"
done < <(git ls-files --others --exclude-standard -z)

SOURCE_SHA="$EXPECTED_SOURCE_SHA" SOURCE_TREE="$EXPECTED_SOURCE_TREE" COMPILER_SHA="$compiler_sha" \
ROOT="$root" INPUT_MANIFEST="$before" EXPECTED_MANIFEST_SHA256="$before_sha" NATIVE_LOG="$log" NATIVE_RECEIPT="$receipt" python3 - <<'PY'
import hashlib,json,os
from pathlib import Path
h=lambda b:hashlib.sha256(b).hexdigest()
root=Path(os.environ['ROOT'])
log=Path(os.environ['NATIVE_LOG']).read_bytes()
art=root/'artifacts/zorde-generated'
plan=json.loads((art/'registry-plan.json').read_text())
data={
 'schema':'oreslang.zorde.source-local.generated-guest/v1',
 'source_head_sha':os.environ['SOURCE_SHA'],
 'source_root_tree_sha':os.environ['SOURCE_TREE'],
 'compiler_executable_sha256':os.environ['COMPILER_SHA'],
 'plan_ir_id':plan['irId'],
 'plan_sha256':h((art/'registry-plan.json').read_bytes()),
 'specs_sha256':h((art/'registry-specs.ores').read_bytes()),
 'smoke_guest_sha256':h((art/'registry-smoke.ores').read_bytes()),
 'native_log_sha256':h(log),
 'pre_execution_input_manifest_sha256':os.environ['EXPECTED_MANIFEST_SHA256'],
 'source_bound_guest_inputs_unchanged':True,
 'native_guest_executed':True,'success_marker_observed':True,
 'process_exit_code':0,
 'compiler_dependency_closure_attested':False,
 'actor_shareability_certified':False,
 'cross_backend_parity_certified':False,
 'release_ready':False,
}
with Path(os.environ['NATIVE_RECEIPT']).open('x',encoding='utf8') as f:
    json.dump(data,f,indent=2,sort_keys=True)
    f.write('\n')
PY
printf 'ZORDE_GENERATED_NATIVE_EXECUTED: %s\n' "$receipt"
