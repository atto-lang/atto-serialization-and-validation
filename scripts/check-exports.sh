#!/usr/bin/env bash
# Static native Oreslang export/import compatibility guard.
# Complements, but does not replace, compiler and ownership checks.
set -euo pipefail
repo_root="$(CDPATH= cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
errors=0
checked=0
while IFS= read -r -d '' file; do
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#"${line%%[![:space:]]*}"}"
    case "$line" in
      'import class '*) kind=class; symbols="${line#import class }" ;;
      'import fnc '*) kind=fnc; symbols="${line#import fnc }" ;;
      *) continue ;;
    esac
    symbols="${symbols%% from *}"
    module="$(printf '%s\n' "$line" | sed -nE "s/.* from '([^']+)'.*/\1/p")"
    if [[ -z "$module" ]]; then
      echo "malformed module import: $file: $line" >&2
      errors=$((errors + 1))
      continue
    fi
    if [[ "$module" == *.ores ]]; then
      target="$(realpath -m "$(dirname "$file")/$module")"
    else
      target="$(realpath -m "$(dirname "$file")/$module.ores")"
    fi
    if [[ ! -f "$target" ]]; then
      echo "import source absent: $file -> $target" >&2
      errors=$((errors + 1))
      continue
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
      if [[ "$kind" == class ]]; then
        signature="^[[:space:]]*pub[[:space:]]+define[[:space:]]+class[[:space:]]+${name}[[:space:]]+as([[:space:]]|$)"
      else
        signature="^[[:space:]]*pub[[:space:]]+((async|nlex|pure)[[:space:]]+)*fnc[[:space:]]+${name}[[:space:]]*\("
      fi
      if ! grep -Eq "$signature" "$target"; then
        echo "unexported imported $kind '$name': $file -> $target" >&2
        errors=$((errors + 1))
      fi
    done
  done < "$file"
done < <(find "$repo_root/src" "$repo_root/tests" -type f -name '*.ores' -print0)
printf 'checked %s imported native declarations; violations=%s\n' "$checked" "$errors"
if [[ "$errors" -ne 0 ]]; then exit 1; fi
