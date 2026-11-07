import json
import pathlib
import re
import unittest
from contracts.strict_json import load_and_validate, MAX_WIRE_BYTES

ROOT = pathlib.Path(__file__).resolve().parents[1] / "contracts" / "v1"
SCHEMA = json.loads((ROOT / "actor-envelope.schema.json").read_text())
WIT = (ROOT / "actor-envelope.wit").read_text()
TSP = (ROOT / "actor-envelope.tsp").read_text()
BASE = {"version": 1, "message_id": "mid-1", "source_actor": "src"}
MSG = {**BASE, "event": "message", "target_actor": "dest", "payload_utf8": "hi"}
FAIL = {**BASE, "event": "failure", "error_code": "IO_ERROR"}
STOP = {**BASE, "event": "shutdown"}

def wit_fields(text):
    m = re.search(r"record\s+actor-envelope\s*\{([^}]*)\}", text)
    if m is None:
        raise AssertionError("missing WIT actor record")
    return dict((k, v.strip()) for k, v in re.findall(r"(?m)^\s*([a-z-]+):\s*([^,\n]+)", m.group(1)))

def tsp_fields(text):
    result = {}
    for name, typ in re.findall(r"(?m)^\s*([a-z][a-z0-9_]*)\s*:\s*([^;\n]+);", text):
        result.setdefault(name, set()).add(typ.strip())
    return result

WIT_EXPECTED = {
    "version": "u32", "event": "event-kind", "message-id": "string",
    "source-actor": "string", "target-actor": "option<string>",
    "payload-utf8": "option<string>", "error-code": "option<string>",
}
TSP_EXPECTED = {
    "version": {"1"}, "message_id": {"string"}, "source_actor": {"string"},
    "target_actor": {"string"}, "payload_utf8": {"string"}, "error_code": {"string"},
    "event": {'"message"', '"failure"', '"shutdown"'},
}
class HardeningTests(unittest.TestCase):
    def test_wit_field_types_and_optional_projection(self):
        self.assertEqual(wit_fields(WIT), WIT_EXPECTED)
        self.assertEqual({k.replace("-", "_") for k in WIT_EXPECTED}, set(SCHEMA["properties"]))

    def test_typespec_member_types(self):
        self.assertEqual(tsp_fields(TSP), TSP_EXPECTED)

    def test_mutated_projection_detected(self):
        self.assertNotEqual(wit_fields(WIT.replace("event: event-kind", "event: string")), WIT_EXPECTED)
        self.assertNotEqual(tsp_fields(TSP.replace("message_id: string;", "message_id: int32;")), TSP_EXPECTED)

    def test_reference_accepts_variants(self):
        for record in (MSG, FAIL, STOP):
            with self.subTest(record=record):
                self.assertEqual(load_and_validate(json.dumps(record), SCHEMA), record)

    def test_type_confusion_matrix(self):
        cases = [(MSG, "message_id"), (MSG, "source_actor"), (MSG, "target_actor"),
                 (FAIL, "error_code"), (MSG, "event")]
        invalid_types = [None, True, 123, 1.5, [], {}, ""]
        checked = 0
        for base, name in cases:
            for value in invalid_types:
                with self.subTest(name=name, value=repr(value)):
                    with self.assertRaises(ValueError):
                        load_and_validate(json.dumps({**base, name: value}), SCHEMA)
                checked += 1
        for value in [None, True, 123, 1.5, [], {}]:
            with self.subTest(payload=value):
                with self.assertRaises(ValueError):
                    load_and_validate(json.dumps({**MSG, "payload_utf8": value}), SCHEMA)
            checked += 1
        self.assertGreaterEqual(checked, 40)

    def test_security_boundaries(self):
        bad = [
            {**MSG, "version": 2}, {**MSG, "message_id": "x" * 129},
            {**MSG, "source_actor": "x" * 129}, {**MSG, "target_actor": "x" * 129},
            {**MSG, "payload_utf8": "x" * 65537}, {**FAIL, "error_code": "A" * 65},
            {**FAIL, "error_code": "IO_ERROR\n"}, {**MSG, "error_code": "IO_ERROR"},
            {**STOP, "target_actor": "dest"}, {**STOP, "payload_utf8": ""},
            {**MSG, "secret": "x"}, {k: v for k, v in MSG.items() if k != "payload_utf8"},
        ]
        for value in bad:
            with self.subTest(value=str(value)[:100]):
                with self.assertRaises(ValueError):
                    load_and_validate(json.dumps(value), SCHEMA)

    def test_ambiguous_json_and_nonstandard_values(self):
        for record in (MSG, FAIL, STOP):
            source = json.dumps(record)
            for name in record:
                raw = source[:-1] + ", " + json.dumps(name) + ": " + json.dumps(record[name]) + "}"
                with self.subTest(name=name):
                    with self.assertRaises(ValueError):
                        load_and_validate(raw, SCHEMA)
        for raw in [b"\xff", b"{", b"[]", b"null", b"{}", b" " * (MAX_WIRE_BYTES+1),
                    json.dumps({**MSG, "version": 1.0}), json.dumps({**MSG, "message_id": "\ud800"}),
                    json.dumps(MSG)[:-1] + ', "extension": NaN}']:
            with self.subTest(raw=repr(raw[:100])):
                with self.assertRaises((ValueError, UnicodeError)):
                    load_and_validate(raw, SCHEMA)

if __name__ == "__main__":
    unittest.main()
