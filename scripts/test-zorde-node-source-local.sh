#!/usr/bin/env bash
# Source-local, exact-tree Node qualification without GitHub Actions minutes.
# Does not export private source, upload artifacts, or certify native Oreslang.
set -euo pipefail
umask 077
# A caller-controlled PATH or GIT_* override must not choose verifier
# utilities or redirect the source snapshot under inspection.
export PATH=/usr/bin:/bin
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR \
  GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_CONFIG_COUNT \
  PYTHONPATH PYTHONHOME PYTHONSTARTUP PYTHONINSPECT
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
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
[[ "$ZORDE_EVIDENCE_DIR" = /* && -d "$ZORDE_EVIDENCE_DIR" && ! -L "$ZORDE_EVIDENCE_DIR" ]] ||
  die 'evidence directory must be an absolute preexisting real directory'
# The final-component -L checks above are insufficient: lstat all ancestor
# components of both independently trusted paths, including the filesystem root.
python3 - "$NODE_BIN" "$ZORDE_EVIDENCE_DIR" <<'PY' || die 'Node executable or private evidence path traverses a symbolic link'
import os,stat,sys
for source,expected_type in ((sys.argv[1],stat.S_ISREG),(sys.argv[2],stat.S_ISDIR)):
    current=os.path.abspath(source)
    first=True
    while True:
        st=os.lstat(current)
        assert not stat.S_ISLNK(st.st_mode),current
        assert expected_type(st.st_mode) if first else stat.S_ISDIR(st.st_mode),current
        first=False
        parent=os.path.dirname(current)
        if parent==current: break
        current=parent
PY
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
[[ "$(git rev-parse --show-toplevel)" == "$root" ]] || die 'source path is not the repository root'
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" ]] || die 'source commit does not match approval'
[[ "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] || die 'source Git tree does not match approval'
git diff --quiet && git diff --cached --quiet || die 'tracked source files were modified'
# Identify the exact tests that are executed, independently of the Git tree.
# Links or unexpected files in the test input path are not admissible.
python3 - "$root" <<'PY' || die 'unsafe or unexpected Node test source files'
import os,stat,sys
root=sys.argv[1]
for rel in ('tests','tests/source_free'):
    st=os.lstat(os.path.join(root,rel))
    assert stat.S_ISDIR(st.st_mode) and not stat.S_ISLNK(st.st_mode)
for rel in ('tests/source_free/test_zorde_contract_ir.mjs',
            'tests/source_free/test_zorde_native_guard.mjs'):
    st=os.lstat(os.path.join(root,rel))
    assert stat.S_ISREG(st.st_mode) and st.st_nlink==1
    assert 0 < st.st_size <= 2*1024*1024
PY
test_ir_sha="$(sha256sum tests/source_free/test_zorde_contract_ir.mjs | cut -d' ' -f1)"
test_guard_sha="$(sha256sum tests/source_free/test_zorde_native_guard.mjs | cut -d' ' -f1)"
[[ "$test_ir_sha" =~ ^[0-9a-f]{64}$ && "$test_guard_sha" =~ ^[0-9a-f]{64}$ ]] || die 'invalid test source digest'
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
# noclobber narrows the filesystem race between checking the fresh filename
# and opening the private test log; never truncate a substituted log or symlink.
set -C
set +e
timeout --signal=TERM --kill-after=5s 300s \
  env -u NODE_OPTIONS -u NODE_PATH -u NODE_V8_COVERAGE -u NODE_EXTRA_CA_CERTS \
  "$NODE_BIN" --test --test-reporter=tap \
  tests/source_free/test_zorde_contract_ir.mjs \
  tests/source_free/test_zorde_native_guard.mjs >"$log" 2>&1
status=$?
set -e
[[ "$status" -eq 0 ]] || die "real Node test suite failed; private log retained (exit $status)"
# Validate TAP only after source/binary rechecks. The final receipt signer
# performs one bounded O_NOFOLLOW read and parses that same verified byte set.
[[ "$(sha256sum "$NODE_BIN" | cut -d' ' -f1)" == "$EXPECTED_NODE_SHA256" ]] ||
  die 'Node binary changed during test execution'
[[ "$(git rev-parse HEAD)" == "$EXPECTED_SOURCE_SHA" &&
   "$(git rev-parse 'HEAD^{tree}')" == "$EXPECTED_SOURCE_TREE" ]] ||
  die 'approved source revision changed during test execution'
git diff --quiet && git diff --cached --quiet || die 'tracked source was modified by tests'
[[ "$(sha256sum tests/source_free/test_zorde_contract_ir.mjs | cut -d' ' -f1)" == "$test_ir_sha" &&
   "$(sha256sum tests/source_free/test_zorde_native_guard.mjs | cut -d' ' -f1)" == "$test_guard_sha" ]] ||
  die 'approved Node test source files changed during execution'
while IFS= read -r -d '' file; do
  [[ "$file" == artifacts/* ]] || die 'test wrote an unexpected untracked file outside artifacts'
done < <(git ls-files --others --exclude-standard -z)
SOURCE_SHA="$EXPECTED_SOURCE_SHA" SOURCE_TREE="$EXPECTED_SOURCE_TREE" \
NODE_SHA="$EXPECTED_NODE_SHA256" NODE_VERSION="$node_version" \
TEST_IR_SHA="$test_ir_sha" TEST_GUARD_SHA="$test_guard_sha" SOURCE_ROOT="$root" \
LOG="$log" RECEIPT="$receipt" python3 - <<'PY' || die 'invalid private Node TAP log or receipt creation was refused'
import hashlib,json,os,re,stat
from pathlib import Path
# Open the exact output log without following symbolic links and prove it is
# a unique, bounded, private file. Hash only the bytes read from this handle.
assert hasattr(os,'O_NOFOLLOW') and os.O_NOFOLLOW != 0
flags=os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK
fd=os.open(os.environ['LOG'],flags)
try:
    before=os.fstat(fd)
    assert stat.S_ISREG(before.st_mode) and before.st_nlink==1
    assert before.st_uid==os.geteuid() and stat.S_IMODE(before.st_mode)&0o077==0
    assert 0 < before.st_size <= 16*1024*1024
    log=os.read(fd,before.st_size+1)
    after=os.fstat(fd)
    assert len(log)==before.st_size==after.st_size
    assert before.st_ino==after.st_ino and before.st_dev==after.st_dev
    assert before.st_mtime_ns==after.st_mtime_ns
finally:
    os.close(fd)
# Revalidate actual test inputs from unique, no-follow file descriptors
# immediately before sealing the final execution receipt.
def checked_test_source(relative, expected):
    parent=os.path.join(os.environ['SOURCE_ROOT'],'tests','source_free')
    st=os.lstat(parent)
    assert stat.S_ISDIR(st.st_mode) and not stat.S_ISLNK(st.st_mode)
    path=os.path.join(parent,relative)
    fd=os.open(path,flags)
    try:
        st=os.fstat(fd)
        assert stat.S_ISREG(st.st_mode) and st.st_nlink==1
        assert 0 < st.st_size <= 2*1024*1024
        data=os.read(fd,st.st_size+1)
        after=os.fstat(fd)
        assert len(data)==st.st_size==after.st_size
        assert st.st_ino==after.st_ino and st.st_mtime_ns==after.st_mtime_ns
        assert hashlib.sha256(data).hexdigest()==expected
    finally:
        os.close(fd)
checked_test_source('test_zorde_contract_ir.mjs',os.environ['TEST_IR_SHA'])
checked_test_source('test_zorde_native_guard.mjs',os.environ['TEST_GUARD_SHA'])
text=log.decode('utf-8')
def metric(name):
    matches=re.findall(r'^# '+name+r' (\d+)\s*$',text,re.M)
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
 'source_free_test_cases_passed':passed,
 'source_free_test_cases_failed':0,
 'source_free_test_cases_skipped':0,
 'node_test_log_sha256':hashlib.sha256(log).hexdigest(),
 'test_source_sha256':{
   'tests/source_free/test_zorde_contract_ir.mjs':os.environ['TEST_IR_SHA'],
   'tests/source_free/test_zorde_native_guard.mjs':os.environ['TEST_GUARD_SHA'],
 },
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
