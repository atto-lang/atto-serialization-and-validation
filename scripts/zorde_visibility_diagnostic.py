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
STORAGE_REFERENCE = re.compile(
    r"(?<![A-Za-z0-9_])(?:"
    r"ZordeScalarRegistry[ \t]*(?:(?:\.|::)[ \t]*|(?:member|field)[ \t]+)entries"
    r"|(?:member|field)[ \t]+entries[ \t]+(?:of|in|on)[ \t]+ZordeScalarRegistry"
    r"|entries[ \t]+(?:of|in|on)[ \t]+ZordeScalarRegistry"
    r")(?![A-Za-z0-9_])"
)
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


def denial_matcher(symbol: str) -> re.Pattern[str]:
    """Bind one visibility-denial phrase to exactly the named protected symbol.

    The symbol remains case-sensitive while compiler wording is case-insensitive.
    Quotes in compiler messages are turned into spacing before matching.
    """
    name = r"(?<![A-Za-z0-9_])" + re.escape(symbol) + r"(?![A-Za-z0-9_])"
    suffix = (r"(?:is[ \t]+private|has[ \t]+private[ \t]+access"
              r"|(?:is[ \t]+)?not[ \t]+(?:exported|public|visible|accessible)"
              r"|is[ \t]+inaccessible|cannot[ \t]+be[ \t]+accessed)")
    prefix = (r"(?:cannot[ \t]+(?:access|import)"
              r"|access[ \t]+denied(?:[ \t]+to)?"
              r"|private[ \t]+(?:class|type|member|field|declaration|symbol|import))")
    descriptor = r"(?:(?:member|field|class|type|symbol)[ \t]+)?"
    return re.compile(
        r"(?:%s[ \t]+(?i:%s)|(?i:%s)[ \t]+(?i:%s)%s)" %
        (name, suffix, prefix, descriptor, name)
    )


BOUND_DENIAL = {
    fixture: denial_matcher("ZordePrivateCompiledEntry" if fixture == "private-entry-import" else "entries")
    for fixture in TARGETS
}
QUOTES = str.maketrans({"'": " ", '"': " ", "`": " "})


def assess(log: bytes, fixture: str) -> tuple[bool, str]:
    """Deny any mixed failure; accept only target-bound visibility diagnostics.

    For a multiline compiler emission, accept an *isolated* two-line form:
    'error: private type' + 'ZordePrivateCompiledEntry'. Do not join generic
    adjacent lines or accept a different declaration's private error.
    """
    if fixture not in TARGETS:
        return False, "unrecognized fixture"
    if not log or len(log) > MAX_LOG_BYTES or b"\x00" in log:
        return False, "empty, binary or oversized diagnostic"
    try:
        message = log.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False, "non-UTF-8 diagnostic"
    message = ANSI_ESCAPE.sub("", message)
    if UNRELATED_FAILURE.search(message):
        return False, "mixed or unrelated compiler failure"
    lines = message.splitlines()
    rule = BOUND_DENIAL[fixture]
    target = TARGETS[fixture]
    for index, line in enumerate(lines):
        if len(line) > 4096:
            continue
        cleaned = line.translate(QUOTES)
        if rule.search(cleaned):
            if fixture == "private-storage-access" and not STORAGE_REFERENCE.search(cleaned):
                continue
            return True, "targeted visibility denial"
        if index + 1 >= len(lines):
            continue
        # Only the explicit compiler two-line grammar may cross the line.
        # A free-floating unrelated private error on an adjacent line cannot.
        if re.fullmatch(r"[ \t]*(?:error:[ \t]*)?private[ \t]+(?:type|class|field|member|symbol)[ \t]*", cleaned, re.I):
            continuation = lines[index + 1].translate(QUOTES).strip()
            if len(continuation) <= 256:
                if fixture == "private-storage-access":
                    if STORAGE_REFERENCE.fullmatch(continuation):
                        return True, "targeted multiline visibility denial"
                elif target.fullmatch(continuation):
                    return True, "targeted multiline visibility denial"
    return False, "targeted access denial absent"


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
