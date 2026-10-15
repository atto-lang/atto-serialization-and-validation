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
        "fnc message_schema_id() => String;",
        "fnc encode_actor_message(borrow T value) => Result<List<int>, EncodeError>;",
        "fnc decode_actor_message(borrow List<int> bytes) => Result<T, DecodeError>;",
    ),
}
DECL = re.compile(
    r"(?m)^\s*(?:pub\s+)?define\s+interface\s+(NetworkCodec|ActorMessageCodec)"
    r"\s*<\s*T\s*>\s+as\s*\n(.*?)^\s*end\s*$",
    re.DOTALL | re.MULTILINE,
)
ANY_DECL = re.compile(
    r"\b(?:define\s+(?:pub\s+)?(?:class|interface|trait)|"
    r"pub\s+define\s+(?:class|interface|trait)|type)\s+"
    r"(NetworkCodec|ActorMessageCodec)\b"
)


def normalize_signature(signature: str) -> str:
    return re.sub(r"\s+", " ", signature.strip())


def validate_interface_source(text: str) -> list[str]:
    """Inspect declarations only, not runtime correctness of codecs."""
    code = "\n".join(line.split("//", 1)[0] for line in text.splitlines())
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
        code = "\n".join(line.split("//", 1)[0] for line in
                         other.read_text(encoding="utf-8").splitlines())
        if ANY_DECL.search(code):
            errors.append(f"{other.relative_to(root)}: cannot redeclare codec contracts")
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
