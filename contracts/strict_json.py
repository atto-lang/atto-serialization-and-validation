"""Strict UTF-8 JSON boundary: reference decoder only, not a production adapter."""
import json
from jsonschema import Draft202012Validator, FormatChecker

MAX_WIRE_BYTES = 1_048_576

def load_and_validate(raw, schema):
    if isinstance(raw, str):
        raw = raw.encode("utf-8", "strict")
    if not isinstance(raw, bytes):
        raise TypeError("expected UTF-8 bytes or string")
    if len(raw) > MAX_WIRE_BYTES:
        raise ValueError("oversized JSON input")
    def unique_keys(pairs):
        obj = {}
        for key, val in pairs:
            if key in obj:
                raise ValueError("duplicate JSON key: " + key)
            obj[key] = val
        return obj
    def reject_float(token):
        raise ValueError("nonintegral or noncanonical JSON number")
    def reject_nonfinite(token):
        raise ValueError("nonfinite number")
    data = json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=unique_keys,
                      parse_float=reject_float, parse_constant=reject_nonfinite)
    if not isinstance(data, dict):
        raise ValueError("expected JSON object")
    try:
        json.dumps(data, ensure_ascii=False).encode("utf-8", "strict")
    except UnicodeEncodeError as exc:
        raise ValueError("invalid Unicode surrogate") from exc
    Draft202012Validator.check_schema(schema)
    errors = list(Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(data))
    if errors:
        raise ValueError(errors[0].message)
    return data
