"""Fail-closed static contract guard; compiler/runtime proof remains separate."""

from __future__ import annotations

import re
from pathlib import Path

INTERFACE_FILE = Path("src/schema/transport_interfaces.ores")
EXPECTED = {
    "NetworkCodec": (
        "fnc encode_network(borrow T value) => Result<List<int>, EncodeError>;",
        "fnc decode_network(borrow List<int> bytes) => Result<T, DecodeError>;",
    ),
    "ActorMessageCodec": (
        "fnc encode_actor_message(borrow T value) => Result<List<int>, EncodeError>;",
        "fnc decode_actor_message(borrow List<int> bytes) => Result<T, DecodeError>;",
    ),
}
DECL = re.compile(
    r"(?m)^\s*(?:pub\s+)?define\s+interface\s+(NetworkCodec|ActorMessageCodec)"
    r"\s*<\s*T\s*>\s+as\s*\n(.*?)^\s*end\s*$",
    re.DOTALL | re.MULTILINE,
)
# An actor codec/CONTROL name must not be recreated as a struct, enum,
# nominal type alias, actor, module or contract after the two required
# declarations. This is name-reservation lint, not guest-code registration.
DECLARATION_KIND = r"(?:class|interface|trait|struct|enum|pattern|module|contract|actor)"
DECLARATION_PREFIX = (
    r"\b(?:(?:define\s+(?:pub\s+)?|pub\s+define\s+)"
    + DECLARATION_KIND + r"|type)\s+"
)
ANY_DECL = re.compile(
    DECLARATION_PREFIX + r"(NetworkCodec|ActorMessageCodec)\b"
)
CONTROL_DECL = re.compile(
    DECLARATION_PREFIX + r"(?:SupervisorControlCodec|ControlCodec)\b"
)


def code_without_comments_or_strings(text: str) -> str:
    """Erase comments and quoted payloads, preserving lines and identifiers.

    Never let a disabled declaration satisfy codec admission. This is an
    intentionally conservative lexical guard, not an Oreslang parser.
    """
    out: list[str] = []
    i = 0
    state = "code"
    quote = ""
    block_depth = 0
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if state == "code":
            if ch == "/" and nxt in ("/", "*"):
                state = "line" if nxt == "/" else "block"
                if nxt == "*":
                    block_depth = 1
                out.extend("  ")
                i += 2
                continue
            # The compiler treats backtick as a token, not a string quote.
            if ch in ("'", '"'):
                state, quote = "quoted", ch
                out.append(" ")
            else:
                out.append(ch)
        elif state == "line":
            out.append("\n" if ch == "\n" else " ")
            if ch == "\n":
                state = "code"
        elif state == "block":
            # The Java/GraalVM lexer nests block comments. A single-depth
            # stripper can incorrectly activate source still inside the
            # outer comment and accept forged codec declarations.
            if ch == "/" and nxt == "*":
                block_depth += 1
                out.extend("  ")
                i += 2
                continue
            if ch == "*" and nxt == "/":
                block_depth -= 1
                out.extend("  ")
                i += 2
                if block_depth == 0:
                    state = "code"
                continue
            out.append("\n" if ch == "\n" else " ")
        elif state == "quoted":
            if ch == "\\":
                out.append(" ")
                if nxt:
                    out.append("\n" if nxt == "\n" else " ")
                    i += 2
                    continue
            elif ch == quote:
                state = "code"
            out.append("\n" if ch == "\n" else " ")
        i += 1
    if state in ("block", "quoted"):
        raise ValueError(f"unterminated {state} construct in codec contract source")
    return "".join(out)


def normalize_signature(signature: str) -> str:
    return re.sub(r"\s+", " ", signature.strip())


def validate_interface_source(text: str) -> list[str]:
    """Inspect declarations only, not runtime correctness of codecs."""
    try:
        code = code_without_comments_or_strings(text)
    except ValueError as exc:
        return [str(exc)]
    matches = list(DECL.finditer(code))
    errors: list[str] = []
    found: set[str] = set()
    for match in matches:
        name = match.group(1)
        if name in found:
            errors.append(f"duplicate {name} interface")
        found.add(name)
        methods = [
            normalize_signature(row)
            for row in match.group(2).splitlines()
            if row.strip()
        ]
        if methods != [normalize_signature(m) for m in EXPECTED[name]]:
            errors.append(f"{name}: changed codec signature, method, order, or ownership mode")
    for name in EXPECTED:
        if name not in found:
            errors.append(f"{name}: required explicit interface missing")
    if len(list(ANY_DECL.finditer(code))) != len(matches):
        errors.append("codec names must be declared once each as generic interfaces")
    if CONTROL_DECL.search(code):
        errors.append("supervisor CONTROL codec is reserved to the runtime")
    return errors


def audit_transport_contracts(root: Path) -> list[str]:
    path = root / INTERFACE_FILE
    if not path.is_file():
        return [f"{INTERFACE_FILE}: missing dual-codec declarations"]
    errors = [f"{INTERFACE_FILE}: {error}" for error in
              validate_interface_source(path.read_text(encoding="utf-8"))]
    for other in (root / "src").rglob("*.ores"):
        if other == path:
            continue
        try:
            code = code_without_comments_or_strings(other.read_text(encoding="utf-8"))
        except ValueError as exc:
            errors.append(f"{other.relative_to(root)}: {exc}")
            continue
        if ANY_DECL.search(code):
            errors.append(f"{other.relative_to(root)}: cannot redeclare codec contracts")
        if CONTROL_DECL.search(code):
            errors.append(f"{other.relative_to(root)}: supervisor CONTROL codec is reserved")
    return errors


if __name__ == "__main__":
    import sys
    root = Path(__file__).resolve().parents[1]
    failures = audit_transport_contracts(root)
    for failure in failures:
        print(failure, file=sys.stderr)
    if failures:
        sys.exit(1)
    print("network and actor-message codec declaration contracts: OK")
