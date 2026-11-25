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


def accepted(log: bytes, fixture: str) -> bool:
    if not log or len(log) > MAX_LOG or b"\0" in log:
        return False
    try:
        content = log.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return False
    if UNRELATED.search(content):
        return False
    target = TARGETS.get(fixture)
    constructor = CONSTRUCTORS.get(fixture)
    if target:
        symbol = re.escape(target)
        body = rf"(?:{symbol}[ \t]+(?i:is private|is not exported|is not accessible|is inaccessible)|(?i:cannot access|cannot import|private class|private type|private member|private field)[ \t]+{symbol})"
    elif constructor:
        symbol = re.escape(constructor)
        body = rf"(?i:constructor)[ \t]+{symbol}[ \t]+(?i:argument count mismatch|argument type mismatch|no matching overload|is not accessible|is private)"
    else:
        return False
    rule = re.compile(r"[ \t]*(?i:error):[ \t]+" + body + r"[.!]?[ \t]*")
    for line in content.splitlines():
        if len(line.encode("utf-8")) <= 4096 and rule.fullmatch(line.translate(str.maketrans({"'": "", '"': "", "`": ""}))):
            return True
    return False
