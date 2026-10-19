#!/usr/bin/env python3
"""Fail-closed review of compiler visibility diagnostics, never guest evidence.

Called only after an independently pinned Oreslang compiler exits nonzero.
No code is imported from a private Oreslang repository.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

MAX_LOG_BYTES = 65536
TARGETS = {
    "private-entry-import": re.compile(r"(?<![A-Za-z0-9_])ZordePrivateCompiledEntry(?![A-Za-z0-9_])"),
    "private-storage-access": re.compile(r"(?<![A-Za-z0-9_])entries(?![A-Za-z0-9_])"),
}
ANSI_ESCAPE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
DENIAL = re.compile(
    r"\b(?:not[\s-]+(?:exported|public|visible|accessible)"
    r"|cannot[\s-]+(?:be[\s-]+)?access(?:ed)?"
    r"|access[\s-]+denied|inaccessible"
    r"|visibility[\s-]+(?:error|denied)"
    r"|is[\s-]+private"
    r"|private[\s-]+(?:class|type|member|field|declaration|symbol|import))\b",
    re.IGNORECASE,
)
UNRELATED_FAILURE = re.compile(
    r"\b(?:syntax error|parse error|unexpected token|internal compiler error"
    r"|internal error|no such file|module not found|class not found"
    r"|exception in thread|segmentation fault|stack trace|panic)\b",
    re.IGNORECASE,
)


def assess(log: bytes, fixture: str) -> tuple[bool, str]:
    """Only matching symbol + access denial on same or adjacent lines qualifies."""
    target = TARGETS.get(fixture)
    if target is None:
        return False, "unrecognized fixture"
    if not log or len(log) > MAX_LOG_BYTES:
        return False, "empty or oversized diagnostic"
    try:
        message = log.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False, "non-UTF-8 diagnostic"
    message = ANSI_ESCAPE.sub("", message)
    if UNRELATED_FAILURE.search(message):
        return False, "mixed or unrelated compiler failure"
    lines = message.splitlines()
    denial_lines = [i for i, line in enumerate(lines) if DENIAL.search(line)]
    symbol_lines = {i for i, line in enumerate(lines) if target.search(line)}
    if not denial_lines or not symbol_lines:
        return False, "targeted access denial absent"
    if not any(any(abs(d - s) <= 1 for s in symbol_lines) for d in denial_lines):
        return False, "access denial separated from protected symbol"
    return True, "targeted visibility denial"


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[2] not in TARGETS:
        print("usage: zorde_visibility_diagnostic.py LOG {private-entry-import|private-storage-access}", file=sys.stderr)
        return 2
    try:
        path = Path(argv[1])
        if not path.is_file() or path.stat().st_size > MAX_LOG_BYTES:
            raise ValueError("missing or oversized log")
        content = path.read_bytes()
    except (OSError, ValueError) as error:
        print(f"visibility evidence rejected: {error}", file=sys.stderr)
        return 2
    accepted, reason = assess(content, argv[2])
    if not accepted:
        print(f"visibility evidence rejected: {reason}", file=sys.stderr)
        return 2
    print(f"visibility diagnostic verified: {argv[2]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
