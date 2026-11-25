#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/scripts" "$tmp/src" "$tmp/tests"
cp "$root/scripts/check-exports.sh" "$tmp/scripts/check-exports.sh"
cat > "$tmp/src/api.ores" <<'EOF'
pub fnc visible(): void {
}
fnc private_fn(): void {
}
EOF
fixture="$tmp/tests/test_std_harness_regressions.ores"
cat > "$fixture" <<'EOF'
import fnc visible from "../src/api";
import fnc visible from '../src/api';
import class TestCase from "ores.std/testing";
import fnc require_success from 'ores.std/testing';
EOF
bash "$tmp/scripts/check-exports.sh" > "$tmp/log"
grep -F 'checked 2 local imported native declarations; external compiler verification pending=2; violations=0' "$tmp/log"
reject() {
  printf '%s\n' "$1" > "$fixture"
  if bash "$tmp/scripts/check-exports.sh" > "$tmp/log" 2>&1; then
    echo "unexpected import admission: $1" >&2
    exit 1
  fi
  grep -F "$2" "$tmp/log"
}
reject 'import fnc private_fn from "../src/api";' 'unexported imported fnc'
reject 'import fnc visible from "../src/missing";' 'import source absent'
reject 'import fnc hidden from "ores.std/testing";' 'unrecognized external import'
reject 'import class require_success from "ores.std/testing";' 'unrecognized external import'
reject 'import fnc require_success from "ores.std/other";' 'import source absent'
reject "import fnc visible from '../src/api\";" 'malformed module import'
reject 'import fnc visible from "../src/api"; extra' 'malformed module import'
reject 'import fnc visible, from "../src/api";' 'unsupported imported identifier list'
printf '%s\n' 'import fnc require_success from "ores.std/testing";' > "$tmp/tests/other.ores"
: > "$fixture"
if bash "$tmp/scripts/check-exports.sh" > "$tmp/log" 2>&1; then
  echo 'external fixture exemption escaped its scope' >&2
  exit 1
fi
grep -F 'import source absent' "$tmp/log"
echo 'import closure checker regressions: PASS'
