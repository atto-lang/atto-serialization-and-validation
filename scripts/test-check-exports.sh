#!/usr/bin/env bash
set -euo pipefail
root="$(CDPATH= cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/scripts" "$tmp/src" "$tmp/tests"
cp "$root/scripts/check-exports.sh" "$tmp/scripts/"
printf '%s\n' 'pub fnc good(): int { return 1; }' > "$tmp/src/local.ores"
fixture="$tmp/tests/test_std_harness_regressions.ores"
printf '%s\n' "import fnc good from '../src/local';" 'import fnc good from "../src/local";' 'import class TestCase from "ores.std/testing";' > "$fixture"
bash "$tmp/scripts/check-exports.sh"
deny() {
  printf '%s\n' "$1" > "$fixture"
  if bash "$tmp/scripts/check-exports.sh"; then echo "unexpected accepted import" >&2; exit 1; fi
}
deny 'import fnc absent from "../src/local";'
deny 'import fnc good from "../src/missing";'
deny 'import fnc arbitrary from "ores.std/testing";'
deny 'import fnc good from "ores.std/unknown";'
deny "import fnc good from '../src/local\";"
rm "$fixture"
printf '%s\n' 'import class TestCase from "ores.std/testing";' > "$tmp/tests/other.ores"
if bash "$tmp/scripts/check-exports.sh"; then echo "external exception escaped harness" >&2; exit 1; fi
echo 'import closure mutation checks passed'
