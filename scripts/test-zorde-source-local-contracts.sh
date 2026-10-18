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
printf 'source-free test fixture\n' > "$src/fixture.txt"
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
mkdir -p "$src/inside-evidence"
expect_refusal in-tree-evidence 'evidence must be outside the source checkout' ZORDE_EVIDENCE_DIR="$src/inside-evidence"
rmdir "$src/inside-evidence"
printf 'dirty\n' > "$src/new-untracked.txt"
expect_refusal dirty-checkout 'source tree is not clean'
rm "$src/new-untracked.txt"
touch "$out/zorde-registry-native.log"
expect_refusal existing-evidence 'evidence output already exists'
rm "$out/zorde-registry-native.log"
chmod 777 "$out"
expect_refusal unsafe-permissions 'evidence directory must be owned by caller and not group/world writable'
chmod 700 "$out"
ln -s "$out" "$tmp/evidence-link"
expect_refusal symlink-evidence 'evidence directory must not be a symlink' ZORDE_EVIDENCE_DIR="$tmp/evidence-link"
printf 'Zorde source-local preflight refusal contracts: %s PASS (no native compiler executed)\n' "$count"
