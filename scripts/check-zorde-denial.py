#!/usr/bin/env python3
"""Require a single access-control diagnostic naming the exact protected symbol.

This is a syntactic evidence filter, not a compiler identity attestation.
"""
import re
import sys
from pathlib import Path

MAX_LOG_BYTES = 1_048_576
TARGETS = {
    "private-entry-import": re.compile(r"\bZordePrivateCompiledEntry\b"),
    "private-storage-access": re.compile(r"\bentries\b"),
}
DENIAL = re.compile(
    r"\b(?:not[\s-]+(?:exported|public|visible|accessible)"
    r"|cannot[\s-]+(?:access|import)"
    r"|access[\s-]+denied"
    r"|inaccessible"
    r"|visibility[\s-]+(?:error|denied)"
    r"|is[\s-]+private"
    r"|private[\s-]+(?:class|type|member|field|declaration|symbol|import))\b",
    re.IGNORECASE,
)

def valid_log(content: bytes, fixture: str) -> bool:
    target = TARGETS.get(fixture)
    if target is None or not content or len(content) > MAX_LOG_BYTES or b"\x00" in content:
        return False
    try:
        data = content.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False
    for line in data.splitlines():
        if len(line) <= 4096 and target.search(line) and DENIAL.search(line):
            return True
    return False

def main() -> int:
    if len(sys.argv) != 3 or sys.argv[2] not in TARGETS:
        return 2
    try:
        path = Path(sys.argv[1])
        if not path.is_file() or path.stat().st_size > MAX_LOG_BYTES:
            return 1
        content = path.read_bytes()
    except OSError:
        return 1
    return 0 if valid_log(content, sys.argv[2]) else 1

if __name__ == "__main__":
    sys.exit(main())
