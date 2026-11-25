"""Source-free diagnostic classifier; not proof of compiler execution."""
import re

TARGETS = {
    "json-frame-visibility/private-state-import": "JsonPrivateFrameState",
    "json-frame-visibility/private-state-mutation": "JsonIncrementalDecoder.state",
    "sealed-schema/private-storage-import": "PrivateObjectSchemaStorage",
    "sealed-schema/private-storage-access": "SealedObjectSchema.storage",
}
CONSTRUCTORS = {
    "json-frame-visibility/legacy-constructor": "JsonIncrementalDecoder",
    "sealed-schema/direct-constructor": "SealedObjectSchema",
}
MAX_LOG = 65536
UNRELATED = re.compile(r"\b(?:syntax error|parse error|unexpected token|internal (?:compiler )?error|no such file|module not found|class not found|exception in thread|segmentation fault|stack trace|panic)\b", re.I)


def exact_symbol(name: str) -> str:
    # Only a balanced quote pair enclosing the whole identifier is allowed.
    # Never delete characters from diagnostic text to manufacture a match.
    literal = re.escape(name)
    return "(?:" + literal + "|'" + literal + "'|\"" + literal + "\"|`" + literal + "`)"


def accepted(log: bytes, fixture: str) -> bool:
    if not log or len(log) > MAX_LOG or b"\0" in log:
        return False
    try:
        content = log.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False
    if any(ord(ch) < 32 and ch not in "\t\r\n" for ch in content):
        return False
    if UNRELATED.search(content):
        return False
    target = TARGETS.get(fixture)
    constructor = CONSTRUCTORS.get(fixture)
    if target:
        symbol = exact_symbol(target)
        body = rf"(?:{symbol}[ \t]+(?i:is private|is not exported|is not accessible|is inaccessible)|(?i:cannot access|cannot import|private class|private type|private member|private field)[ \t]+{symbol})"
    elif constructor:
        symbol = exact_symbol(constructor)
        body = rf"(?i:constructor)[ \t]+{symbol}[ \t]+(?i:argument count mismatch|argument type mismatch|no matching overload|is not accessible|is private)"
    else:
        return False
    rule = re.compile(r"[ \t]*(?i:error):[ \t]+" + body + r"[.!]?[ \t]*")
    matches = 0
    for line in content.replace("\r\n", "\n").split("\n"):
        if len(line.encode("utf-8")) > 4096 or "\r" in line:
            return False
        if rule.fullmatch(line):
            matches += 1
        elif re.match(r"[ \t]*(?i:error):", line):
            return False
    return matches == 1
