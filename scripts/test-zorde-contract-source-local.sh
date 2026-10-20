#!/usr/bin/env bash
# Approved, private source-local static Contract IR qualification.
# No cross-org checkout, mirror, publishing, credential, or artifact upload.
# This does not compile/execute the Oreslang guest or certify backend parity.
set -euo pipefail
umask 077
export PATH=/usr/bin:/bin
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CONFIG_COUNT \
  PYTHONPATH PYTHONHOME PYTHONSTARTUP PYTHONINSPECT \
  NODE_OPTIONS NODE_PATH NODE_V8_COVERAGE NODE_EXTRA_CA_CERTS
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
refuse(){ printf 'ZORDE_CONTRACT_SOURCE_LOCAL_REFUSED: %s\n' "$*" >&2; exit 2; }
for utility in git sha256sum timeout python3; do
  command -v "$utility" >/dev/null 2>&1 || refuse "missing verifier utility: $utility"
done
for var in EXPECTED_SOURCE_SHA EXPECTED_SOURCE_TREE EXPECTED_NODE_SHA256 NODE_BIN TJSV_ROOT ZORDE_EVIDENCE_DIR; do
  [[ -n "$(printenv "$var" || true)" ]] || refuse "missing independently approved input: $var"
done
[[ "$EXPECTED_SOURCE_SHA" =~ ^[0-9a-f]{40}$ && "$EXPECTED_SOURCE_TREE" =~ ^[0-9a-f]{40}$ ]] || refuse 'invalid source SHA/tree'
[[ "$EXPECTED_NODE_SHA256" =~ ^[0-9a-f]{64}$ ]] || refuse 'invalid approved Node SHA-256'
# Exactly the same public TJSV version as the existing Zorde workflow.
tjsv_sha=a9aa7e0a258a0514b433178842288ab798b0947c
tjsv_tree=414047074b9358b81e79b5995a1dc4c1c5596a48
[[ "$NODE_BIN" == /* && -f "$NODE_BIN" && -x "$NODE_BIN" && ! -L "$NODE_BIN" ]] || refuse 'Node must be an absolute non-symlink executable'
[[ "$TJSV_ROOT" == /* && -d "$TJSV_ROOT" && ! -L "$TJSV_ROOT" ]] || refuse 'TJSV must be an absolute directory, not a link'
[[ "$ZORDE_EVIDENCE_DIR" == /* && -d "$ZORDE_EVIDENCE_DIR" && ! -L "$ZORDE_EVIDENCE_DIR" ]] || refuse 'private evidence directory must exist and be absolute'
python3 - "$NODE_BIN" "$TJSV_ROOT" "$ZORDE_EVIDENCE_DIR" <<'PY' || refuse 'approved executable/validator/evidence path traverses symlink'
import os,stat,sys
for p,want in ((sys.argv[1],stat.S_ISREG),(sys.argv[2],stat.S_ISDIR),(sys.argv[3],stat.S_ISDIR)):
    q=os.path.abspath(p)
    first=True
    while True:
        st=os.lstat(q)
        assert not stat.S_ISLNK(st.st_mode),q
        assert (want(st.st_mode) if first else stat.S_ISDIR(st.st_mode)),q
        first=False
        parent=os.path.dirname(q)
        if parent==q: break
        q=parent
PY
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
[[ "$(git rev-parse --show-toplevel)" == "$root" ]] || refuse 'checkout root mismatch'
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" && "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] || refuse 'wrong private source commit/tree'
git diff --quiet && git diff --cached --quiet || refuse 'source checkout has modified tracked files'
while IFS= read -r -d '' file; do
  [[ "$file" == artifacts/* ]] || refuse "unexpected untracked source file"
done < <(git ls-files --others --exclude-standard -z)
[[ "$(sha256sum "$NODE_BIN" | cut -d' ' -f1)" == "$EXPECTED_NODE_SHA256" ]] || refuse 'Node executable digest differs from reviewed input'
version="$(env -u NODE_OPTIONS -u NODE_PATH "$NODE_BIN" --version)"
if [[ "$version" =~ ^v22\.([0-9]+)\.([0-9]+)$ ]]; then
  (( 10#${BASH_REMATCH[1]} >= 18 )) || refuse 'Node 22.18+ is required by the pinned TJSV toolchain'
else
  refuse 'Node 22.18+ is required by the pinned TJSV toolchain'
fi
tjsv_root="$(cd "$TJSV_ROOT" && pwd -P)"
[[ "$tjsv_root" != "$root" && "$tjsv_root" != "$root/"* ]] || refuse 'TJSV must be outside private source tree'
[[ "$(git -C "$tjsv_root" rev-parse --show-toplevel)" == "$tjsv_root" ]] || refuse 'TJSV checkout root mismatch'
[[ "$(git -C "$tjsv_root" rev-parse HEAD)" == "$tjsv_sha" && "$(git -C "$tjsv_root" rev-parse 'HEAD^{tree}')" == "$tjsv_tree" ]] || refuse 'public TJSV source revision mismatch'
git -C "$tjsv_root" diff --quiet && git -C "$tjsv_root" diff --cached --quiet || refuse 'public TJSV tracked source modified'
[[ -f "$tjsv_root/bin/typespec-json-schema-validator.mjs" && -f "$tjsv_root/scripts/verify-consumer.mjs" ]] || refuse 'approved TJSV entrypoints missing'
[[ -f "$tjsv_root/node_modules/@typespec/compiler/package.json" && -f "$tjsv_root/node_modules/@typespec/json-schema/package.json" ]] || refuse 'TJSV preinstalled dependencies missing (run independently approved npm ci on trusted machine)'
evidence="$(cd "$ZORDE_EVIDENCE_DIR" && pwd -P)"
[[ "$evidence" != "$root" && "$evidence" != "$root/"* ]] || refuse 'evidence directory cannot reside in private source'
python3 - "$evidence" <<'PY' || refuse 'evidence directory ownership and privacy invalid'
import os,stat,sys
s=os.lstat(sys.argv[1])
assert stat.S_ISDIR(s.st_mode) and s.st_uid==os.geteuid() and (stat.S_IMODE(s.st_mode)&0o077)==0
PY
# A fresh private evidence area must be used for each exact source/validator pair.
[[ ! -e "$evidence/zorde-node-real-suite-receipt.json" && ! -L "$evidence/zorde-node-real-suite-receipt.json" ]] || refuse 'use a fresh evidence directory (Node receipt already present)'
[[ ! -e "$evidence/zorde-contract-static.log" && ! -L "$evidence/zorde-contract-static.log" && ! -e "$evidence/zorde-contract-static-receipt.json" && ! -L "$evidence/zorde-contract-static-receipt.json" ]] || refuse 'stale Contract IR log/receipt'
[[ ! -e artifacts/zorde-scalars-parity.json && ! -e artifacts/zorde-generated && ! -e artifacts/zorde-scalars-witness && ! -e artifacts/zorde-scalars-contract-ir.json ]] || refuse 'stale generated Contract IR or source directory; use a fresh checkout'
# Native Node crypto/filesystem tests are mandatory before using external TJSV.
bash scripts/test-zorde-node-source-local.sh || refuse 'actual private Node security tests did not pass'
[[ -f "$evidence/zorde-node-real-suite-receipt.json" ]] || refuse 'real Node execution receipt missing'
# Runtime output is private; do not upload or publish it.
set -C
run_timeout(){ timeout --signal=TERM --kill-after=5s 300s "$@"; }
log="$evidence/zorde-contract-static.log"
exec 3>"$log"
{
  run_timeout "$NODE_BIN" "$tjsv_root/bin/typespec-json-schema-validator.mjs" check \
    --typespec=contracts/zorde/scalars.tsp \
    --schema=contracts/zorde/scalars.schema.json \
    --instances=contracts/zorde/scalar-instances \
    --report=artifacts/zorde-scalars-parity.json \
    --contract-ir=artifacts/zorde-scalars-contract-ir.json \
    --output-dir=artifacts/zorde-scalars-witness \
    --max-probes=64 \
    --int64-strategy=string \
    --seal-object-schemas=true \
    --polymorphic-models-strategy=oneOf || refuse 'peer authority TypeSpec/JSON Schema check failed'
  TSJSV_VERIFY_IR=artifacts/zorde-scalars-contract-ir.json \
    TSJSV_VERIFY_REPORT=artifacts/zorde-scalars-parity.json \
    TSJSV_VERIFY_TYPESPEC=contracts/zorde/scalars.tsp \
    TSJSV_VERIFY_AUTHORED=contracts/zorde/scalars.schema.json \
    TSJSV_VERIFY_GENERATED=artifacts/zorde-scalars-witness \
    TSJSV_VERIFY_DECLARATIONS='["Zorde.Validation.Port","Zorde.Validation.ShortLabel"]' \
    TSJSV_VERIFY_VERIFICATION=artifacts/zorde-scalars-consumer-verification.json \
    GITHUB_WORKSPACE="$root" \
    run_timeout "$NODE_BIN" "$tjsv_root/scripts/verify-consumer.mjs" || refuse 'source-bound Contract IR verifier rejected evidence'
  ir_sha="$(sha256sum artifacts/zorde-scalars-contract-ir.json | cut -d' ' -f1)"
  report_sha="$(sha256sum artifacts/zorde-scalars-parity.json | cut -d' ' -f1)"
  run_timeout "$NODE_BIN" scripts/zorde-static-contract-ir.mjs \
    --ir=artifacts/zorde-scalars-contract-ir.json \
    --report=artifacts/zorde-scalars-parity.json \
    --ir-sha256="$ir_sha" --report-sha256="$report_sha" \
    --instances=contracts/zorde/scalar-instances \
    --out-dir=artifacts/zorde-generated || refuse 'static registry generation failed'
} >&3 2>&3
exec 3>&-
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" && "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] || refuse 'source snapshot changed during Contract IR execution'
[[ "$(git -C "$tjsv_root" rev-parse HEAD)" == "$tjsv_sha" && "$(git -C "$tjsv_root" rev-parse 'HEAD^{tree}')" == "$tjsv_tree" ]] || refuse 'TJSV snapshot changed during Contract IR execution'
git diff --quiet && git diff --cached --quiet || refuse 'private tracked files modified during Contract IR execution'
git -C "$tjsv_root" diff --quiet && git -C "$tjsv_root" diff --cached --quiet || refuse 'TJSV tracked files modified during Contract IR execution'
[[ "$(sha256sum "$NODE_BIN" | cut -d' ' -f1)" == "$EXPECTED_NODE_SHA256" ]] || refuse 'Node binary changed during contract verification'
# Seal an explicitly NON-NATIVE receipt from real source-local execution.
SOURCE="$EXPECTED_SOURCE_SHA" TREE="$EXPECTED_SOURCE_TREE" TJSV="$tjsv_sha" TJSV_TREE="$tjsv_tree" NODE_SHA="$EXPECTED_NODE_SHA256" EVIDENCE="$evidence" python3 - <<'PY' || refuse 'static Contract IR receipt refused'
import hashlib,json,os,stat
from pathlib import Path
root=Path.cwd()
evidence=Path(os.environ['EVIDENCE'])
sha=lambda b: hashlib.sha256(b).hexdigest()
assert hasattr(os,'O_NOFOLLOW') and os.O_NOFOLLOW != 0

def file(p):
    # Refuse symbolic-link substitutions through the full selected path,
    # not only the leaf. Use exactly the bytes admitted by fstat to hash.
    p=Path(os.path.abspath(p))
    for parent in p.parents:
        st=os.lstat(parent)
        assert stat.S_ISDIR(st.st_mode) and not stat.S_ISLNK(st.st_mode)
    fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
    try:
        before=os.fstat(fd)
        assert stat.S_ISREG(before.st_mode) and before.st_nlink==1
        assert before.st_uid==os.geteuid() and 0 < before.st_size <= 8*1024*1024
        data=os.read(fd,before.st_size+1)
        after=os.fstat(fd)
        assert len(data)==before.st_size==after.st_size
        assert before.st_dev==after.st_dev and before.st_ino==after.st_ino
        assert before.st_mtime_ns==after.st_mtime_ns
        return data
    finally:
        os.close(fd)
node_receipt=json.loads(file(evidence/'zorde-node-real-suite-receipt.json'))
assert node_receipt['source_head_sha']==os.environ['SOURCE']
assert node_receipt['source_root_tree_sha']==os.environ['TREE']
assert node_receipt['node_executable_sha256']==os.environ['NODE_SHA']
assert node_receipt['real_node_crypto_filesystem_tests_executed'] is True
assert node_receipt['source_free_test_cases_failed']==node_receipt['source_free_test_cases_skipped']==0
assert node_receipt['release_ready'] is False
assert sha(file(evidence/'zorde-node-real-suite.log'))==node_receipt['node_test_log_sha256']
files={
 'typespec':'contracts/zorde/scalars.tsp',
 'authoredJsonSchema':'contracts/zorde/scalars.schema.json',
 'reviewedParity':'artifacts/zorde-scalars-parity.json',
 'contractIr':'artifacts/zorde-scalars-contract-ir.json',
 'consumerVerification':'artifacts/zorde-scalars-consumer-verification.json',
 'guestPlan':'artifacts/zorde-generated/registry-plan.json',
 'guestSpecs':'artifacts/zorde-generated/registry-specs.ores',
 'guestSmoke':'artifacts/zorde-generated/registry-smoke.ores',
 'buildComplete':'artifacts/zorde-generated/build-complete.json',
}
digests={name:sha(file(root/path)) for name,path in files.items()}
parity=json.loads(file(root/files['reviewedParity']))
verification=json.loads(file(root/files['consumerVerification']))
plan=json.loads(file(root/files['guestPlan']))
build=json.loads(file(root/files['buildComplete']))
assert parity['status']=='passed' and parity['zeroUnexplainedFindings'] is True
assert verification['status']=='passed' and verification['admissible'] is True
assert verification['computedIrId']==verification['expectedIrId']==verification['suppliedIrId']
assert plan['irId']==verification['expectedIrId']==build['irId']
assert build['reviewedCorpusDigest']==parity['configuration']['differential']['instanceCorpusDigest']
assert build['reviewedCorpusInstances']==9
assert plan['releaseReady'] is False and build['releaseReady'] is False
assert build['guestSmokeCompiled'] is False and build['guestSmokeExecuted'] is False
receipt={
 'schema':'oreslang.zorde.source-local.static-contract-ir/v1',
 'source_head_sha':os.environ['SOURCE'],'source_root_tree_sha':os.environ['TREE'],
 'public_tjsv_head_sha':os.environ['TJSV'],'public_tjsv_root_tree_sha':os.environ['TJSV_TREE'],
 'source_local_harness_sha256':sha(file(root/'scripts/test-zorde-contract-source-local.sh')),
 'approved_node_executable_sha256':os.environ['NODE_SHA'],
 'node_receipt_sha256':sha(file(evidence/'zorde-node-real-suite-receipt.json')),
 'static_execution_log_sha256':sha(file(evidence/'zorde-contract-static.log')),
 'static_input_and_output_sha256':digests,
 'real_node_crypto_filesystem_tests_executed':True,
 'typespec_compiler_and_peer_parity_executed':True,
 'source_bound_contract_ir_consumer_verification_executed':True,
 'static_registry_source_generated':True,
 'native_guest_compiled':False,'native_guest_executed':False,
 'compiler_dependency_closure_attested':False,'cross_backend_parity_certified':False,
 'release_ready':False,
}
with (evidence/'zorde-contract-static-receipt.json').open('x',encoding='utf8') as f:
    json.dump(receipt,f,sort_keys=True,indent=2);f.write('\n')
PY
printf 'ZORDE_SOURCE_LOCAL_STATIC_CONTRACT_IR_PASSED (not native): %s\n' "$evidence/zorde-contract-static-receipt.json"
