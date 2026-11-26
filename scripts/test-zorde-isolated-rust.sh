#!/usr/bin/env bash
set -euo pipefail
root="$(pwd -P)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
rustc --edition=2021 -D warnings "$root/scripts/zorde-isolated.rs" -o "$tmp/isolated"
rustc --edition=2021 -D warnings --test "$root/scripts/zorde-isolated.rs" -o "$tmp/unit"
"$tmp/unit"
mkdir -m 700 "$tmp/source" "$tmp/source/scripts"
cat >"$tmp/startup" <<EOF
printf injected > '$tmp/injected'
EOF
# Inert approved gate, not a guest.
cat >"$tmp/source/scripts/test-zorde-contract-source-local.sh" <<'SH'
set -eu
for key in BASH_ENV ENV NODE_OPTIONS PYTHONPATH GIT_DIR INJECTED; do
  if printenv "$key" >/dev/null; then exit 7; fi
done
test "$PATH" = /usr/bin:/bin
test -n "$EXPECTED_SOURCE_SHA"
printf 'synthetic gate only\n'
SH
cp "$tmp/source/scripts/test-zorde-contract-source-local.sh" "$tmp/source/scripts/test-zorde-generated-guest-source-local.sh"
hash="$(sha256sum "$tmp/source/scripts/test-zorde-contract-source-local.sh" | cut -d' ' -f1)"
export EXPECTED_SOURCE_SHA="$(printf 'a%.0s' {1..40})" EXPECTED_SOURCE_TREE="$(printf 'b%.0s' {1..40})"
export EXPECTED_NODE_SHA256="$(printf 'c%.0s' {1..64})" NODE_BIN=/synthetic/node TJSV_ROOT=/synthetic/tjsv ZORDE_EVIDENCE_DIR=/synthetic/evidence
export EXPECTED_COMPILER_SHA256="$EXPECTED_NODE_SHA256" EXPECTED_CONTRACT_IR_SHA256="$EXPECTED_NODE_SHA256"
export EXPECTED_PARITY_REPORT_SHA256="$EXPECTED_NODE_SHA256" EXPECTED_VERIFICATION_SHA256="$EXPECTED_NODE_SHA256" ORESLANG_BIN=/synthetic/compiler
n=0
deny(){
  local rc=0
  "$@" >"$tmp/out" 2>&1 || rc=$?
  test "$rc" = 2 || { cat "$tmp/out"; exit 1; }
  grep -Fq ZORDE_ISOLATED_REFUSED "$tmp/out"
  n=$((n+1))
}
for mode in contract guest; do
  env BASH_ENV="$tmp/startup" ENV="$tmp/startup" NODE_OPTIONS=--inspect PYTHONPATH=/bad GIT_DIR=/bad INJECTED=1 \
    "$tmp/isolated" bootstrap "$mode" "$tmp/source" "$hash"
  test ! -e "$tmp/injected"
  n=$((n+1))
done
deny env -u EXPECTED_NODE_SHA256 "$tmp/isolated" bootstrap guest "$tmp/source" "$hash"
deny env EXPECTED_NODE_SHA256=bad "$tmp/isolated" bootstrap guest "$tmp/source" "$hash"
deny "$tmp/isolated" bootstrap contract "$tmp/source" "$EXPECTED_NODE_SHA256"
ln -s "$tmp/source" "$tmp/source-alias"
deny "$tmp/isolated" bootstrap contract "$tmp/source-alias" "$hash"
rm "$tmp/source-alias"
ln "$tmp/source/scripts/test-zorde-contract-source-local.sh" "$tmp/linked"
deny "$tmp/isolated" bootstrap contract "$tmp/source" "$hash"
rm "$tmp/linked"
printf '#!/bin/sh\nprintf "synthetic success\\n"\n' >"$tmp/good"
printf '#!/bin/sh\nexit 7\n' >"$tmp/bad"
printf '#!/bin/sh\nsleep 30\n' >"$tmp/slow"
printf '#!/bin/sh\nhead -c 20000000 /dev/zero || :\nexit 0\n' >"$tmp/large"
chmod 700 "$tmp/good" "$tmp/bad" "$tmp/slow" "$tmp/large"
printf 'synthetic fixture only\n' >"$tmp/guest"
log=zorde-generated-smoke-native.log
for mode in fresh regular fifo symlink dangling hardlink alias public nonzero timeout saturated; do
  d="$tmp/$mode"; mkdir -m 700 "$d"; c="$tmp/good"; dest="$d"; seconds=5
  case "$mode" in
    regular) printf preserve >"$d/$log";;
    fifo) mkfifo "$d/$log";;
    symlink) printf preserve >"$d/protected"; ln -s "$d/protected" "$d/$log";;
    dangling) ln -s "$d/absent" "$d/$log";;
    hardlink) printf preserve >"$d/protected"; ln "$d/protected" "$d/$log";;
    alias) ln -s "$d" "$tmp/log-alias";dest="$tmp/log-alias";;
    public) chmod 750 "$d";;
    nonzero) c="$tmp/bad";;
    timeout) c="$tmp/slow";seconds=1;;
    saturated) c="$tmp/large";;
  esac
  if [[ "$mode" == fresh ]]; then
    "$tmp/isolated" capture "$dest" "$c" "$tmp/guest" "$seconds"
    grep -Fxq 'synthetic success' "$d/$log";test "$(stat -c %a "$d/$log")" = 600
    n=$((n+1))
  else
    deny "$tmp/isolated" capture "$dest" "$c" "$tmp/guest" "$seconds"
  fi
  case "$mode" in
    regular) test "$(cat "$d/$log")" = preserve;;
    symlink|hardlink) test "$(cat "$d/protected")" = preserve;;
    dangling) test ! -e "$d/absent";;
    saturated) test "$(stat -c %s "$d/$log")" -le 16777216;;
  esac
done
printf 'ISOLATED_SYNTHETIC_CONTROLS=%s PASS\n' "$n"
