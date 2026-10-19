#!/usr/bin/env bash
# Source-local, exact-tree Node qualification without GitHub Actions minutes.
# Does not export private source, upload artifacts, or certify native Oreslang.
set -euo pipefail
umask 077
die(){ printf 'ZORDE_NODE_QUALIFICATION_REFUSED: %s\n' "$*" >&2; exit 2; }
for utility in git sha256sum timeout python3 env; do
  command -v "$utility" >/dev/null 2>&1 || die "required utility unavailable: $utility"
done
for variable in EXPECTED_SOURCE_SHA EXPECTED_SOURCE_TREE EXPECTED_NODE_SHA256 NODE_BIN ZORDE_EVIDENCE_DIR; do
  [[ -n "$(printenv "$variable" || true)" ]] || die "required reviewed input missing: $variable"
done
[[ "$EXPECTED_SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]] || die 'invalid immutable source commit'
[[ "$EXPECTED_SOURCE_TREE" =~ ^[0-9a-f]{40}$ ]] || die 'invalid immutable source root tree'
[[ "$EXPECTED_NODE_SHA256" =~ ^[0-9a-f]{64}$ ]] || die 'invalid independently approved Node binary digest'
[[ "$NODE_BIN" = /* && -f "$NODE_BIN" && -x "$NODE_BIN" && ! -L "$NODE_BIN" ]] ||
  die 'Node must be an absolute real executable, not a symlink'
[[ -d "$ZORDE_EVIDENCE_DIR" && ! -L "$ZORDE_EVIDENCE_DIR" ]] ||
  die 'evidence directory must preexist, without a symlink'
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
[[ "$(git rev-parse --show-toplevel)" == "$root" ]] || die 'source path is not the repository root'
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" ]] || die 'source commit does not match approval'
[[ "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] || die 'source Git tree does not match approval'
git diff --quiet && git diff --cached --quiet || die 'tracked source files were modified'
while IFS= read -r -d '' file; do
  [[ "$file" == artifacts/* ]] || die 'unexpected untracked file outside generated artifacts'
done < <(git ls-files --others --exclude-standard -z)
node_sha="$(sha256sum "$NODE_BIN" | cut -d' ' -f1)"
[[ "$node_sha" == "$EXPECTED_NODE_SHA256" ]] || die 'Node binary digest does not match approval'
node_version="$(env -u NODE_OPTIONS -u NODE_PATH "$NODE_BIN" --version)"
[[ "$node_version" =~ ^v22\. ]] || die 'Node 22 required for pinned source-free test runner'
out="$(cd "$ZORDE_EVIDENCE_DIR" && pwd -P)"
case "$out" in "$root"|"$root"/*) die 'evidence directory must be outside checkout';; esac
python3 - "$out" <<'PY' || die 'unsafe evidence directory policy'
import os,stat,sys
s=os.lstat(sys.argv[1])
assert stat.S_ISDIR(s.st_mode) and s.st_uid==os.geteuid()
assert stat.S_IMODE(s.st_mode)&0o077==0
PY
log="$out/zorde-node-real-suite.log"
receipt="$out/zorde-node-real-suite-receipt.json"
[[ ! -e "$log" && ! -L "$log" && ! -e "$receipt" && ! -L "$receipt" ]] ||
  die 'node log and receipt must not already exist'
set +e
timeout --signal=TERM --kill-after=5s 300s \
  env -u NODE_OPTIONS -u NODE_PATH -u NODE_V8_COVERAGE -u NODE_EXTRA_CA_CERTS \
  "$NODE_BIN" --test --test-reporter=tap \
  tests/source_free/test_zorde_contract_ir.mjs \
  tests/source_free/test_zorde_native_guard.mjs >"$log" 2>&1
status=$?
set -e
[[ "$status" -eq 0 ]] || die "real Node test suite failed; private log retained (exit $status)"
python3 - "$log" <<'PY' || die 'invalid, incomplete or unexpectedly small Node TAP results'
import re,sys
s=open(sys.argv[1],encoding='utf-8').read()
def metric(name):
    hits=re.findall(r'^# '+name+r' (\d+)\s*$',s,re.MULTILINE)
    assert len(hits)==1,(name,hits)
    return int(hits[0])
n=metric('tests');passed=metric('pass');fail=metric('fail')
skip=metric('skipped');todo=metric('todo');cancelled=metric('cancelled')
assert n>=80 and passed==n and fail==0 and skip==0 and todo==0 and cancelled==0, (n,passed,fail,skip,todo,cancelled)
PY
[[ "$(sha256sum "$NODE_BIN" | cut -d' ' -f1)" == "$EXPECTED_NODE_SHA256" ]] ||
  die 'Node binary changed during test execution'
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" &&
   "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] ||
  die 'approved source revision changed during test execution'
git diff --quiet && git diff --cached --quiet || die 'tracked source was modified by tests'
while IFS= read -r -d '' file; do
  [[ "$file" == artifacts/* ]] || die 'test wrote an unexpected untracked file outside artifacts'
done < <(git ls-files --others --exclude-standard -z)
SOURCE_SHA="$EXPECTED_SOURCE_SHA" SOURCE_TREE="$EXPECTED_SOURCE_TREE" \
NODE_SHA="$EXPECTED_NODE_SHA256" NODE_VERSION="$node_version" \
LOG="$log" RECEIPT="$receipt" python3 - <<'PY'
import hashlib,json,os,re
from pathlib import Path
log=Path(os.environ['LOG']).read_bytes()
text=log.decode('utf-8')
def metric(name):
    matches=re.findall(r'^# '+name+r' (\d+)\s*
receipt={
 'schema':'oreslang.zorde.source-local.node-security-suite/v1',
 'source_head_sha':os.environ['SOURCE_SHA'],
 'source_root_tree_sha':os.environ['SOURCE_TREE'],
 'node_executable_sha256':os.environ['NODE_SHA'],
 'node_version':os.environ['NODE_VERSION'],
 'source_free_test_cases_passed':passed,
 'source_free_test_cases_failed':0,
 'source_free_test_cases_skipped':0,
 'node_test_log_sha256':hashlib.sha256(log).hexdigest(),
 'real_node_crypto_filesystem_tests_executed':True,
 'generated_oreslang_guest_executed':False,
 'compiler_dependency_closure_attested':False,
 'cross_backend_parity_certified':False,
 'release_ready':False,
}
with Path(os.environ['RECEIPT']).open('x',encoding='utf-8') as f:
    json.dump(receipt,f,sort_keys=True,indent=2)
    f.write('\n')
PY
printf 'ZORDE_NODE_SOURCE_LOCAL_PASSED: %s\n' "$receipt"
,text,re.M)
    assert len(matches)==1,(name,matches)
    return int(matches[0])
tests=metric('tests');passed=metric('pass');failed=metric('fail')
skipped=metric('skipped');todo=metric('todo');cancelled=metric('cancelled')
assert tests>=80 and passed==tests and failed==0 and skipped==0 and todo==0 and cancelled==0
receipt={
 'schema':'oreslang.zorde.source-local.node-security-suite/v1',
 'source_head_sha':os.environ['SOURCE_SHA'],
 'source_root_tree_sha':os.environ['SOURCE_TREE'],
 'node_executable_sha256':os.environ['NODE_SHA'],
 'node_version':os.environ['NODE_VERSION'],
 'source_free_test_cases_passed':tests,
 'source_free_test_cases_failed':0,
 'source_free_test_cases_skipped':0,
 'node_test_log_sha256':hashlib.sha256(log).hexdigest(),
 'real_node_crypto_filesystem_tests_executed':True,
 'generated_oreslang_guest_executed':False,
 'compiler_dependency_closure_attested':False,
 'cross_backend_parity_certified':False,
 'release_ready':False,
}
with Path(os.environ['RECEIPT']).open('x',encoding='utf-8') as f:
    json.dump(receipt,f,sort_keys=True,indent=2)
    f.write('\n')
PY
printf 'ZORDE_NODE_SOURCE_LOCAL_PASSED: %s\n' "$receipt"
