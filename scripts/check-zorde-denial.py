#!/usr/bin/env python3
"""Require a single access-control diagnostic naming the exact protected symbol.

This is a syntactic evidence filter, not a compiler identity attestation.
"""
import re
import sys
from pathlib import Path

MAX_LOG_BYTES = 1_048_576
def denial_matcher(symbol: str) -> re.Pattern:
    """Accept a visibility denial only if the protected symbol is its subject/object.

    Finding a name and unrelated denial on the same diagnostic line is not proof.
    """
    target = r"(?<!\w)[`'\\"]?" + re.escape(symbol) + r"[`'\\"]?(?!\w)"
    suffix = (r"(?:is[ \t]+private|has[ \t]+private[ \t]+access"
              r"|(?:is[ \t]+)?not[ \t]+(?:exported|public|visible|accessible)"
              r"|is[ \t]+inaccessible|cannot[ \t]+be[ \t]+accessed)")
    prefix = (r"(?:cannot[ \t]+(?:access|import)|access[ \t]+denied[ \t]+(?:to[ \t]+)?"
              r"|private[ \t]+(?:class|type|member|field|declaration|symbol|import))")
    descriptor = r"(?:(?:member|field|class|type|symbol)[ \t]+)?"
    return re.compile(r"(?:%s[ \t]+%s|%s[ \t]+%s%s)" %
                      (target, suffix, prefix, descriptor, target), re.IGNORECASE)


TARGETS = {
    "private-entry-import": denial_matcher("ZordePrivateCompiledEntry"),
    "private-storage-access": denial_matcher("entries"),
}

def valid_log(content: bytes, fixture: str) -> bool:
    target = TARGETS.get(fixture)
    if target is None or not content or len(content) > MAX_LOG_BYTES or b"\x00" in content:
        return False
    try:
        data = content.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False
    for line in data.splitlines():
        if len(line) <= 4096 and target.search(line):
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
