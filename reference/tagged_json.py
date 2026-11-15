"""Strict Option/Result tagged JSON reference (host only, not wire standard)."""
import json
import math
import re

MAX_BYTES = 65536
MAX_DEPTH = 32
_CODE = re.compile("[a-z][a-z0-9_]{0,63}", re.ASCII)


class TaggedCodecError(ValueError):
    pass


def _object(pairs):
    obj = {}
    for key, value in pairs:
        if key in obj:
            raise TaggedCodecError("duplicate JSON key")
        obj[key] = value
    return obj


def _check_value(value, depth=0):
    if depth > MAX_DEPTH:
        raise TaggedCodecError("nested value exceeds depth limit")
    if value is None or type(value) in (str, bool, int):
        return
    if type(value) is float and math.isfinite(value):
        return
    if type(value) is list:
        for element in value:
            _check_value(element, depth + 1)
        return
    if type(value) is dict and all(type(key) is str for key in value):
        for element in value.values():
            _check_value(element, depth + 1)
        return
    raise TaggedCodecError("unsupported or nonfinite JSON value")


def validate_tagged(value):
    if type(value) is not dict or value.get("kind") not in ("option", "result"):
        raise TaggedCodecError("expected tagged option/result object")
    kind, variant = value["kind"], value.get("variant")
    if kind == "option" and variant == "none":
        required = {"kind", "variant"}
    elif kind == "option" and variant == "some":
        required = {"kind", "variant", "value"}
    elif kind == "result" and variant == "ok":
        required = {"kind", "variant", "value"}
    elif kind == "result" and variant == "err":
        required = {"kind", "variant", "error"}
    else:
        raise TaggedCodecError("invalid kind/variant pairing")
    if set(value) != required:
        raise TaggedCodecError("missing or unexpected tagged fields")
    if "value" in required:
        _check_value(value["value"])
    if variant == "err":
        err = value["error"]
        if type(err) is not dict or set(err) != {"code", "message"}:
            raise TaggedCodecError("error requires code and message")
        if type(err["code"]) is not str or not _CODE.fullmatch(err["code"]):
            raise TaggedCodecError("invalid error code")
        if type(err["message"]) is not str or len(err["message"]) > 512:
            raise TaggedCodecError("invalid error message")
    return value


def encode_tagged(value):
    validate_tagged(value)
    try:
        raw = json.dumps(value, ensure_ascii=False, allow_nan=False,
                         separators=(",", ":"), sort_keys=True).encode("utf-8")
    except (UnicodeError, TypeError, ValueError, RecursionError) as exc:
        raise TaggedCodecError("cannot encode tagged JSON") from exc
    if len(raw) > MAX_BYTES:
        raise TaggedCodecError("tagged JSON exceeds byte budget")
    return raw


def decode_tagged(raw):
    if type(raw) is not bytes or len(raw) > MAX_BYTES:
        raise TaggedCodecError("input must be bounded bytes")
    try:
        obj = json.loads(raw.decode("utf-8"), object_pairs_hook=_object,
                         parse_constant=lambda x: (_ for _ in ()).throw(TaggedCodecError("nonfinite JSON")))
    except (UnicodeError, json.JSONDecodeError, RecursionError, ValueError) as exc:
        raise TaggedCodecError("invalid tagged JSON") from exc
    return validate_tagged(obj)
