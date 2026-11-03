#!/usr/bin/env python3
"""Source-only serde vector drift check; NEVER reports native guest PASS."""
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "fixtures/serde/adversarial-v1.json"
SUITES = {"json": "tests/json_test.ores", "messagepack": "tests/messagepack_test.ores"}


def check() -> None:
    raw = MANIFEST.read_bytes()
    data = json.loads(raw)
    if data.get("schema") != "attolang.serde.adversarial-v1":
        raise ValueError("unsupported fixture manifest schema")
    if data.get("source_suites") != SUITES:
        raise ValueError("guest suite paths drifted")
    main = (ROOT / data["guest_entrypoint"]).read_text()
    for symbol in ("run_json_tests", "run_messagepack_tests"):
        if not re.search(r"\b" + symbol + r"\s*\(\s*\)", main):
            raise ValueError(f"{symbol} is not invoked by guest entrypoint")

    cases = data["cases"]
    if len(cases) != 21:
        raise ValueError(f"expected 21 exact guest vectors, got {len(cases)}")
    seen = set()
    counts = {"Err": 0, "Ok": 0}
    sources = {fmt: (ROOT / path).read_text() for fmt, path in SUITES.items()}

    for case in cases:
        case_id = case["id"]
        fmt, symbol, expected = case["format"], case["guest_symbol"], case["expected_result"]
        if case_id in seen or case_id != f"{fmt}.{symbol}" or fmt not in SUITES:
            raise ValueError(f"duplicate/invalid case id: {case_id}")
        seen.add(case_id)
        if expected not in counts:
            raise ValueError(f"invalid Result expectation: {case_id}")
        counts[expected] += 1
        if expected == "Err" and not case.get("expected_error_code"):
            raise ValueError(f"error code not specified: {case_id}")
        if expected == "Ok" and "expected_error_code" in case:
            raise ValueError(f"Ok case declares error code: {case_id}")

        hex_bytes = case["bytes_hex"]
        if not re.fullmatch(r"(?:[0-9a-f]{2})+", hex_bytes):
            raise ValueError(f"invalid canonical bytes: {case_id}")
        source = sources[fmt]
        fixture = re.search(
            r"\bval\s+Result<DecodedValue,\s*DecodeError>\s+"
            + re.escape(symbol)
            + r"\s*=\s*decode(?:_default)?\s*\(\s*new List<int>\s*\(([^)]*)\)",
            source,
            re.S,
        )
        if not fixture:
            raise ValueError(f"guest fixture missing: {case_id}")
        literals = fixture.group(1)
        if not re.fullmatch(r"\s*\d+(?:\s*,\s*\d+)*\s*", literals):
            raise ValueError(f"nonliteral guest fixture bytes: {case_id}")
        actual = bytes(int(part.strip()) for part in literals.split(","))
        if actual.hex() != hex_bytes:
            raise ValueError(f"manifest/guest bytes mismatch: {case_id}")
        predicate = r"\brequire_true\s*\(\s*" + ("!" if expected == "Ok" else "")
        predicate += re.escape(symbol) + r"\.is_err\s*\(\s*\)"
        if not re.search(predicate, source, re.S):
            raise ValueError(f"guest Result assertion missing: {case_id}")

    if counts != {"Err": 19, "Ok": 2}:
        raise ValueError(f"wrong negative/positive coverage: {counts}")
    print("SOURCE_ONLY fixture audit: PASS (21 source vectors; 19 Err, 2 Ok)")
    print("fixture_manifest_sha256=" + hashlib.sha256(raw).hexdigest())
    print("NATIVE_GUEST_EVIDENCE=NOT_EXECUTED")


if __name__ == "__main__":
    try:
        check()
    except (OSError, KeyError, TypeError, ValueError) as exc:
        print("SOURCE_ONLY fixture audit: FAIL: " + str(exc), file=sys.stderr)
        sys.exit(1)
