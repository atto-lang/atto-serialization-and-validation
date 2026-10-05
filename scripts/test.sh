#!/usr/bin/env sh
set -eu

if [ -n "${ORESLANG_BIN:-}" ]; then
  exec "$ORESLANG_BIN" tests/main.ores
fi

if command -v oreslang >/dev/null 2>&1; then
  exec oreslang tests/main.ores
fi

echo "oreslang compiler/runtime not found; set ORESLANG_BIN or install the oreslang CLI" >&2
exit 2
