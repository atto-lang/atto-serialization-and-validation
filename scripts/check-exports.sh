#!/usr/bin/env bash
# Static native Oreslang export/import compatibility guard.
# Complements, but does not replace, compiler and ownership checks.
set -euo pipefail
repo_root="$(CDPATH= cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
errors=0
checked=0
external_pending=0
while IFS= read -r -d '' file; do
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#"${line%%[![:space:]]*}"}"
    case "$line" in
      'import class '*) kind=class; symbols="${line#import class }" ;;
      'import fnc '*) kind=fnc; symbols="${line#import fnc }" ;;
      *) continue ;;
    esac
    symbols="${symbols%% from *}"
    # Both quote styles are supported; reject malformed or mixed quoting.
    module="$(printf '%s\n' "$line" | sed -nE "s/^[^'\";]* from '([^'\"]+)'[[:space:]]*;[[:space:]]*$/\1/p")"
    if [[ -z "$module" ]]; then
      module="$(printf '%s\n' "$line" | sed -nE 's/^[^"\x27;]* from "([^"\x27]+)"[[:space:]]*;[[:space:]]*$/\1/p')"
    fi
    if [[ -z "$module" ]]; then
      echo "malformed module import: $file: $line" >&2
      errors=$((errors + 1))
      continue
    fi
    if [[ ! "$symbols" =~ ^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*([[:space:]]*,[[:space:]]*[A-Za-z_][A-Za-z0-9_]*)*[[:space:]]*$ ]]; then
      echo "unsupported imported identifier list: $file: $symbols" >&2
      errors=$((errors + 1))
      continue
    fi
    external=0
    if [[ "$module" == "ores.std/testing" && "$file" == "$repo_root/tests/test_std_harness_regressions.ores" ]]; then
      external=1
    else
      if [[ "$module" == *.ores ]]; then
        target="$(dirname "$file")/$module"
      else
        target="$(dirname "$file")/$module.ores"
      fi
      if [[ ! -f "$target" ]]; then
        echo "import source absent: $file -> $target" >&2
        errors=$((errors + 1))
        continue
      fi
    fi
    IFS=',' read -r -a names <<< "$symbols"
    for name in "${names[@]}"; do
      name="${name//[[:space:]]/}"
      checked=$((checked + 1))
      if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
        echo "unsupported imported identifier: $file $name" >&2
        errors=$((errors + 1))
        continue
      fi
      if [[ "$external" -eq 1 ]]; then
        case "$kind:$name" in
          class:TestCase|class:TestSuite|class:TestOptions|class:TestOutcome|class:RunSummary|fnc:expect_true|fnc:expect_false|fnc:expect_equal_int|fnc:expect_equal_string|fnc:run_suite|fnc:require_success|fnc:fail)
            external_pending=$((external_pending + 1)); checked=$((checked - 1)) ;;
          *) echo "unrecognized external import: $file $kind:$name" >&2; errors=$((errors + 1)) ;;
        esac
        continue
      fi
      if [[ "$kind" == class ]]; then
        signature="^[[:space:]]*(pub[[:space:]]+define|define[[:space:]]+pub)[[:space:]]+class[[:space:]]+${name}[[:space:]]+as([[:space:]]|$)"
      else
        signature="^[[:space:]]*pub[[:space:]]+(async[[:space:]]+)?fnc[[:space:]]+${name}[[:space:]]*\("
      fi
      if ! grep -Eq "$signature" "$target"; then
        echo "unexported imported $kind '$name': $file -> $target" >&2
        errors=$((errors + 1))
      fi
    done
  done < "$file"
done < <(find "$repo_root/src" "$repo_root/tests" -type f -name '*.ores' -print0)
printf 'checked %s local imported native declarations; external compiler verification pending=%s; violations=%s\n' "$checked" "$external_pending" "$errors"
if [[ "$errors" -ne 0 ]]; then exit 1; fi
