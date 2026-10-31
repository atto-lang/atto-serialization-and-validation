#!/usr/bin/env bash
# Source-free preflight contract tests. Fake compiler must never produce a
# native evidence receipt, and is never presented as native Oreslang evidence.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/zorde-source-local-contracts.XXXXXXXX")"
trap 'rm -rf "$tmp"' EXIT
src="$tmp/source with spaces"
out="$tmp/evidence"
mkdir -p "$src/scripts" "$out"
chmod 700 "$out"
cp "$root/scripts/test-zorde-registry-source-local.sh" "$src/scripts/"
cp "$root/scripts/verify-zorde-checkout.py" "$src/scripts/"
# The newly hardened native harness invokes this independently tested
# source-free verifier when it checks synthetic negative diagnostics.
# Copy it into the disposable fixture as well; otherwise the intended
# HEAD-change denial is masked by a missing helper script.
cp "$root/scripts/zorde_visibility_diagnostic.py" "$src/scripts/"
# Synthetic-only script: no production source, compiler, or backend is copied.
# It is used solely to prove tampering cannot lead to a receipt.
mkdir -p "$src/tests" "$src/fixtures/zorde-compile-positive"
printf 'synthetic public import source only\n' > "$src/fixtures/zorde-compile-positive/public-registry-import.ores"
cat > "$src/scripts/test.sh" <<'TEST'
#!/bin/sh
set -eu
exec "$ORESLANG_BIN" "$PWD/tests/main.ores"
TEST
printf 'not an Oreslang guest program\n' > "$src/tests/main.ores"
printf 'source-free test fixture\n' > "$src/fixture.txt"
# This ignore rule exists solely to prove that Git's 'clean' status is not
# adequate for source-byte evidence qualification.
printf '*.generated-ignored\n' > "$src/.gitignore"
mkdir -p "$src/nested"
printf 'tracked child bytes\n' > "$src/nested/child.ores"
git -C "$src" init -q
git -C "$src" config user.name 'Zorde harness contract test'
git -C "$src" config user.email 'zorde-contract@example.invalid'
git -C "$src" add .
git -C "$src" commit -qm 'isolated preflight fixture'
head="$(git -C "$src" rev-parse HEAD)"
tree="$(git -C "$src" rev-parse 'HEAD^{tree}')"
# This executable is a NON-COMPILER placeholder used solely for rejecting
# unsafe configurations before the test.sh/native execution stage.
cat > "$tmp/not-a-compiler" <<'COMPILER'
#!/bin/sh
printf 'FAKE COMPILER MUST NOT BE INVOKED\n' >&2
exit 79
COMPILER
chmod 700 "$tmp/not-a-compiler"
compiler_sha="$(sha256sum "$tmp/not-a-compiler" | cut -d ' ' -f 1)"
base=(
  "EXPECTED_SOURCE_SHA=$head"
  "EXPECTED_SOURCE_TREE=$tree"
  "EXPECTED_COMPILER_SHA256=$compiler_sha"
  "ORESLANG_BIN=$tmp/not-a-compiler"
  "ZORDE_EVIDENCE_DIR=$out"
)
count=0
expect_refusal() {
  local label="$1" expected="$2"
  shift 2
  local status=0
  env "${base[@]}" "$@" bash "$src/scripts/test-zorde-registry-source-local.sh" \
    > "$tmp/check-output" 2>&1 || status=$?
  if [[ "$status" -ne 2 ]] || ! grep -Fq "$expected" "$tmp/check-output"; then
    printf 'preflight control failed: %s (exit=%s)\n' "$label" "$status" >&2
    cat "$tmp/check-output" >&2
    exit 1
  fi
  if [[ -e "$out/zorde-registry-native-receipt.json" ]]; then
    printf 'fake compiler test generated a native receipt: %s\n' "$label" >&2
    exit 1
  fi
  count=$((count + 1))
}
expect_refusal missing-source 'EXPECTED_SOURCE_SHA must' EXPECTED_SOURCE_SHA=
expect_refusal stale-head 'checked-out head differs' EXPECTED_SOURCE_SHA=0000000000000000000000000000000000000000
expect_refusal wrong-tree 'checked-out root Git tree differs' EXPECTED_SOURCE_TREE=1111111111111111111111111111111111111111
expect_refusal wrong-compiler 'compiler executable digest differs' EXPECTED_COMPILER_SHA256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
expect_refusal missing-compiler 'approved compiler executable missing' ORESLANG_BIN="$tmp/absent"
ln -s "$tmp/not-a-compiler" "$tmp/compiler-link"
expect_refusal compiler-symlink 'compiler executable must not be a symlink' ORESLANG_BIN="$tmp/compiler-link"
mkdir -p "$tmp/real-bin"
cp "$tmp/not-a-compiler" "$tmp/real-bin/compiler"
ln -s "$tmp/real-bin" "$tmp/alias-bin"
expect_refusal compiler-parent-symlink 'compiler executable path must have no symlink components' ORESLANG_BIN="$tmp/alias-bin/compiler"
mkdir -p "$src/inside-evidence"
expect_refusal in-tree-evidence 'evidence must be outside the source checkout' ZORDE_EVIDENCE_DIR="$src/inside-evidence"
rmdir "$src/inside-evidence"
printf 'dirty\n' > "$src/new-untracked.txt"
expect_refusal dirty-checkout 'source tree is not clean'
rm "$src/new-untracked.txt"

# Physical checkout attestation runs *before* any repository-provided shell
# script on the protected runner. It must reject ordinary untracked files
# independently of the later shell git-status guard.
printf 'unreviewed guest source bytes\n' > "$src/injected-untracked.ores"
if (cd "$src" && python3 scripts/verify-zorde-checkout.py "$head" "$tree") \
    >"$tmp/check-output" 2>&1; then
  echo 'physical verifier accepted ordinary untracked source input' >&2
  exit 1
fi
grep -Fq 'untracked files exist in source checkout' "$tmp/check-output" ||
  { cat "$tmp/check-output" >&2; exit 1; }
[[ ! -e "$out/zorde-registry-native-receipt.json" ]] ||
  { echo 'ordinary untracked test emitted native receipt' >&2; exit 1; }
count=$((count + 1))
rm "$src/injected-untracked.ores"

# Git status can report 'clean' if the index deliberately hides a modified
# tracked source file. Byte-to-tree attestation must reject both mechanisms.
git -C "$src" update-index --assume-unchanged fixture.txt
printf 'hidden source mutation\n' > "$src/fixture.txt"
expect_refusal hidden-assume-unchanged 'source bytes differ from reviewed Git tree'
git -C "$src" update-index --no-assume-unchanged fixture.txt
git -C "$src" restore -- fixture.txt
git -C "$src" update-index --skip-worktree fixture.txt
printf 'hidden source mutation\n' > "$src/fixture.txt"
expect_refusal hidden-skip-worktree 'source bytes differ from reviewed Git tree'
git -C "$src" update-index --no-skip-worktree fixture.txt
git -C "$src" restore -- fixture.txt

# Regression: ignored untracked input is invisible to git status but can
# influence compiler import discovery or tooling during an attested run.
printf 'synthetic ignored material\n' > "$src/injected.generated-ignored"
[[ -z "$(git -C "$src" status --porcelain --untracked-files=all)" ]] ||
  { echo 'synthetic ignored test did not hide injected file' >&2; exit 1; }
expect_refusal ignored-untracked 'source bytes differ from reviewed Git tree'
rm "$src/injected.generated-ignored"

# An untracked directory symlink can redirect a tracked child to external
# storage containing identical bytes. Test the physical attester directly:
# git status may reject the symlink first, but the attester must also refuse.
mv "$src/nested" "$tmp/external-identical-child"
ln -s "$tmp/external-identical-child" "$src/nested"
if (cd "$src" && python3 scripts/verify-zorde-checkout.py "$head" "$tree") >"$tmp/check-output" 2>&1; then
  echo 'redirected tracked parent directory was accepted' >&2
  exit 1
fi
grep -Fq 'tracked parent directory is not a real directory' "$tmp/check-output" ||
  { cat "$tmp/check-output" >&2; exit 1; }
[[ ! -e "$out/zorde-registry-native-receipt.json" ]] ||
  { echo 'redirected parent test emitted a native receipt' >&2; exit 1; }
count=$((count + 1))
rm "$src/nested"
mv "$tmp/external-identical-child" "$src/nested"

touch "$out/zorde-registry-native.log"
expect_refusal existing-evidence 'evidence output already exists'
rm "$out/zorde-registry-native.log"
chmod 755 "$out"
expect_refusal unsafe-permissions 'evidence directory must be caller-owned and inaccessible to group/world'
chmod 700 "$out"
ln -s "$out" "$tmp/evidence-link"
expect_refusal symlink-evidence 'evidence directory must not be a symlink' ZORDE_EVIDENCE_DIR="$tmp/evidence-link"
ln -s "$tmp" "$tmp/alias-root"
expect_refusal evidence-parent-symlink 'evidence directory path must have no symlink components' ZORDE_EVIDENCE_DIR="$tmp/alias-root/evidence"

# Regression: a broken git status that returns nonzero but no stdout is NOT
# proof of a clean checkout. All other Git subcommands use the real binary.
real_git="$(command -v git)"
mkdir -p "$tmp/shadow-git"
cat > "$tmp/shadow-git/git" <<'GIT'
#!/bin/sh
if [ "$1" = status ]; then exit 91; fi
exec "$REAL_GIT" "$@"
GIT
chmod 700 "$tmp/shadow-git/git"
expect_refusal git-status-error 'unable to inspect source working tree' \
  REAL_GIT="$real_git" PATH="$tmp/shadow-git:$PATH"

# Parent-regression reconciliation: a fake binary can echo native suite
# success markers but cannot resolve a public import. Such a result must
# fail before any privacy-denial fixture and before a native receipt.
cat > "$tmp/synthetic-public-failure" <<'FAKE'
#!/bin/sh
case "$1" in
  */tests/main.ores)
    printf '%s\n' 'native Zorde compiled scalar registry tests passed' \
      'serialization/validation native tests passed'
    exit 0 ;;
  */fixtures/zorde-compile-positive/public-registry-import.ores)
    echo 'synthetic public import failure' >&2; exit 47 ;;
  *) exit 48 ;;
esac
FAKE
chmod 700 "$tmp/synthetic-public-failure"
public_failure_sha="$(sha256sum "$tmp/synthetic-public-failure" | cut -d ' ' -f 1)"
expect_refusal public-import-control 'public registry import positive control failed' \
  ORESLANG_BIN="$tmp/synthetic-public-failure" EXPECTED_COMPILER_SHA256="$public_failure_sha"
[[ -f "$out/zorde-public-import-positive.log" ]] ||
  { echo 'public-import failure log absent' >&2; exit 1; }
rm "$out/zorde-registry-native.log" "$out/zorde-public-import-positive.log"

# Regression: a different HEAD with EXACTLY THE SAME Git tree is not the
# reviewed commit. An intentionally fake compiler makes the workflow reach
# the postrun identity gate, then mutates HEAD. This MUST refuse a receipt.
# These markers are synthetic test strings, not evidence of Oreslang execution.
alternate_head="$(printf 'synthetic alternative HEAD\n' |
  git -C "$src" commit-tree "$tree" -p "$head")"
cat > "$tmp/synthetic-head-switcher" <<'FAKE'
#!/bin/sh
set -eu
case "$1" in
  */tests/main.ores)
    git -C "$FAKE_SOURCE" update-ref HEAD "$FAKE_ALT_HEAD"
    printf '%s\n' 'native Zorde compiled scalar registry tests passed' \
      'serialization/validation native tests passed'
    ;;
  */fixtures/zorde-compile-positive/public-registry-import.ores)
    exit 0 ;;
  */fixtures/zorde-compile-positive/public-export-control.ores)
    printf '%s\n' 'native Zorde public export control passed'
    ;;
  */fixtures/zorde-compile-negative/private-entry-import.ores)
    echo 'ZordePrivateCompiledEntry is private' >&2; exit 42
    ;;
  */fixtures/zorde-compile-negative/private-storage-access.ores)
    echo 'ZordeScalarRegistry entries is private' >&2; exit 42
    ;;
  *) echo 'unexpected synthetic fixture' >&2; exit 78 ;;
esac
FAKE
chmod 700 "$tmp/synthetic-head-switcher"
synthetic_sha="$(sha256sum "$tmp/synthetic-head-switcher" | cut -d ' ' -f 1)"
expect_refusal switched-head 'source HEAD changed during native validation' \
  ORESLANG_BIN="$tmp/synthetic-head-switcher" EXPECTED_COMPILER_SHA256="$synthetic_sha" \
  FAKE_SOURCE="$src" FAKE_ALT_HEAD="$alternate_head"
[[ "$(git -C "$src" rev-parse HEAD)" == "$alternate_head" ]] ||
  { echo 'synthetic fixture did not exercise a HEAD switch' >&2; exit 1; }
[[ "$(git -C "$src" rev-parse 'HEAD^{tree}')" == "$tree" ]] ||
  { echo 'synthetic fixture unexpectedly changed Git tree' >&2; exit 1; }
printf 'Zorde source-local preflight refusal contracts: %s PASS (no native compiler executed)\n' "$count"
