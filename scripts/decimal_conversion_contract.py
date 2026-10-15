"""Fail-closed static regression for the native signed decimal converter.

This check protects the known malformed nested-loop/undefined-digit bug.
It does not replace running the actual Oreslang compiler and native tests.
"""
from pathlib import Path
import re

ROUTINE = "pub fnc int_to_decimal_bytes(int value): List<int> {"
NEXT = "pub fnc value_int(int value): Value {"


def validate_decimal_source(source: str) -> list[str]:
    if source.count(ROUTINE) != 1 or source.count(NEXT) != 1:
        return ["decimal codec: expected exactly one known conversion routine"]
    start = source.index(ROUTINE)
    end = source.index(NEXT, start)
    body = "\n".join(row.split("//", 1)[0] for row in source[start:end].splitlines())
    faults: list[str] = []

    if len(re.findall(r"\bloop\s+do\b", body)) != 2 or len(re.findall(r"\bdone\b", body)) != 2:
        faults.append("decimal codec: unexpected loops; a nested or unterminated loop may skip digits")
    if len(re.findall(r"\b(?:let|val|const)\s+List<int>\s+reversed\b", body)) != 1:
        faults.append("decimal codec: reversed accumulator must have exactly one owner")
    if len(re.findall(r"\b(?:let|val|const)\s+int\s+digit\s*=", body)) != 1:
        faults.append("decimal codec: digit must be initialized once before use")
    if not re.search(r"\bquotient\s*=\s*current\s*/\s*10\s*;", body):
        faults.append("decimal codec: missing signed-safe truncating quotient")
    if not re.search(r"\bremainder\s*=\s*current\s*-\s*\(\s*quotient\s*\*\s*10\s*\)\s*;", body):
        faults.append("decimal codec: missing non-overflowing signed remainder")
    # A declared digit is not enough: "let int digit = 0" would silently
    # serialize every nonzero integer incorrectly while passing the old lint.
    if not re.search(
        r"\blet\s+int\s+digit\s*=\s*remainder\s*<\s*0\s*\?\s*0\s*-\s*remainder\s*:\s*remainder\s*;",
        body,
    ):
        faults.append("decimal codec: expected non-overflowing absolute digit")
    if not re.search(r"\breversed\.add\s*\(\s*48\s*\+\s*digit\s*\)\s*;", body):
        faults.append("decimal codec: missing ASCII digit emission")
    if not re.search(r"\bcurrent\s*=\s*quotient\s*;", body):
        faults.append("decimal codec: loop must advance toward zero")
    if re.search(r"\b(?:current|value)\s*=\s*-\s*(?:current|value)\b", body):
        faults.append("decimal codec: negating the int64 minimum is forbidden")
    if not re.search(r"\boutput\.add\s*\(\s*45\s*\)", body):
        faults.append("decimal codec: minus sign must be emitted for negatives")
    if not re.search(r"\boutput\.add\s*\(\s*48\s*\)", body):
        faults.append("decimal codec: decimal zero must be emitted exactly")
    return faults


def audit_decimal_converter(root: Path) -> list[str]:
    path = root / "src/core/value.ores"
    if not path.is_file():
        return [f"{path}: missing native integer codec"]
    return validate_decimal_source(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    import sys
    errors = audit_decimal_converter(Path(__file__).resolve().parents[1])
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        sys.exit(1)
    print("signed int64 native decimal converter static contract: OK")
