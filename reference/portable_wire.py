"""Bounded standard-MessagePack subset and candidate ATB1 frame (HOST ONLY).

Not an Atto runtime, general schema engine, or a full MessagePack implementation.
Supported values: None/bool/int (-2^63..2^64-1)/finite float/UTF-8 text/
bytes/list/string-keyed dict. Other MessagePack extensions fail closed.
"""

import hmac
import re
import struct

MAX_BYTES = 65_536
MAX_DEPTH = 32
MAX_ITEMS = 4_096
_MAGIC = b"ATB1"
_HEADER = 44
_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z", re.ASCII)


class WireError(ValueError):
    pass


def validate_external_names(field_names):
    """Validate internal Struct identifier -> external UTF-8 field-name map."""
    if type(field_names) is not dict:
        raise WireError("field metadata must be a dict")
    seen = set()
    for internal, external in field_names.items():
        if type(internal) is not str or not _IDENT.fullmatch(internal):
            raise WireError("invalid internal Struct identifier")
        if type(external) is not str:
            raise WireError("external field name must be text")
        try:
            encoded = external.encode("utf-8")
        except UnicodeError as exc:
            raise WireError("invalid Unicode field name") from exc
        if len(encoded) > MAX_BYTES:
            raise WireError("field name too large")
        if external in seen:
            raise WireError("colliding external field names")
        seen.add(external)
    return dict(field_names)


def encode_msgpack(value):
    """Deterministic, bounded MessagePack subset; object keys sorted by UTF-8."""
    out = bytearray()
    active = set()
    remaining = MAX_ITEMS

    def emit(data):
        if len(data) > MAX_BYTES - len(out):
            raise WireError("encoded payload exceeds byte budget")
        out.extend(data)

    def header(length, fix_limit, fix_tag, small, medium, large):
        if length < fix_limit:
            emit(bytes([fix_tag | length]))
        elif length < 65536:
            emit(bytes([small]) + struct.pack(">H", length))
        elif length <= MAX_ITEMS:
            emit(bytes([medium]) + struct.pack(">I", length))
        else:
            raise WireError("collection length exceeds budget")

    def write(v, depth):
        nonlocal remaining
        if depth > MAX_DEPTH:
            raise WireError("nesting exceeds depth budget")
        if v is None:
            emit(b"\xc0")
        elif type(v) is bool:
            emit(b"\xc3" if v else b"\xc2")
        elif type(v) is int:
            if not -(1 << 63) <= v < (1 << 64):
                raise WireError("integer outside MessagePack 64-bit range")
            if 0 <= v <= 127:
                emit(bytes([v]))
            elif -32 <= v < 0:
                emit(bytes([v & 255]))
            else:
                if v >= 0:
                    options = ((255, 0xcc, ">B"), (65535, 0xcd, ">H"),
                               (2**32 - 1, 0xce, ">I"), (2**64 - 1, 0xcf, ">Q"))
                else:
                    options = ((127, 0xd0, ">b"), (32767, 0xd1, ">h"),
                               (2**31 - 1, 0xd2, ">i"), (2**63 - 1, 0xd3, ">q"))
                for upper, opcode, fmt in options:
                    if (v <= upper if v >= 0 else v >= -upper - 1):
                        emit(bytes([opcode]) + struct.pack(fmt, v))
                        break
        elif type(v) is float:
            if not (-float("inf") < v < float("inf")):
                raise WireError("nonfinite float")
            emit(b"\xcb" + struct.pack(">d", v))
        elif type(v) is str:
            try:
                raw = v.encode("utf-8")
            except UnicodeError as exc:
                raise WireError("invalid Unicode scalar") from exc
            if len(raw) > MAX_BYTES:
                raise WireError("string too large")
            n = len(raw)
            if n < 32:
                emit(bytes([0xa0 | n]))
            elif n < 256:
                emit(b"\xd9" + bytes([n]))
            elif n < 65536:
                emit(b"\xda" + struct.pack(">H", n))
            else:
                emit(b"\xdb" + struct.pack(">I", n))
            emit(raw)
        elif type(v) is bytes:
            n = len(v)
            if n > MAX_BYTES:
                raise WireError("binary too large")
            if n < 256:
                emit(b"\xc4" + bytes([n]))
            elif n < 65536:
                emit(b"\xc5" + struct.pack(">H", n))
            else:
                emit(b"\xc6" + struct.pack(">I", n))
            emit(v)
        elif type(v) in (list, dict):
            if id(v) in active:
                raise WireError("cyclic container cannot be serialized")
            active.add(id(v))
            try:
                n = len(v)
                remaining -= n
                if remaining < 0 or n > MAX_ITEMS:
                    raise WireError("collection item budget exceeded")
                if type(v) is list:
                    header(n, 16, 0x90, 0xdc, 0xdd, 0xdd)
                    for child in v:
                        write(child, depth + 1)
                else:
                    if any(type(key) is not str for key in v):
                        raise WireError("only string-keyed maps supported; use typed entry schema")
                    header(n, 16, 0x80, 0xde, 0xdf, 0xdf)
                    try:
                        keys = sorted(v, key=lambda key: key.encode("utf-8"))
                    except UnicodeError as exc:
                        raise WireError("invalid Unicode key") from exc
                    for key in keys:
                        write(key, depth + 1)
                        write(v[key], depth + 1)
            finally:
                active.remove(id(v))
        else:
            raise WireError("unsupported value type")

    write(value, 0)
    return bytes(out)


def decode_msgpack(raw):
    """Strict full-consumption decode of supported MessagePack subset."""
    if type(raw) is not bytes or len(raw) > MAX_BYTES:
        raise WireError("input must be bounded bytes")
    i = 0
    remaining = MAX_ITEMS

    def take(n):
        nonlocal i
        if n < 0 or n > len(raw) - i:
            raise WireError("truncated MessagePack value")
        data = raw[i:i+n]
        i += n
        return data

    def number(n, signed=False):
        return int.from_bytes(take(n), "big", signed=signed)

    def text(n):
        try:
            return take(n).decode("utf-8")
        except UnicodeError as exc:
            raise WireError("invalid UTF-8 string") from exc

    def sequence(n, mapping, depth):
        nonlocal remaining
        remaining -= n
        if remaining < 0 or n > MAX_ITEMS:
            raise WireError("collection item budget exceeded")
        if mapping:
            obj = {}
            for _ in range(n):
                key = read(depth + 1)
                if type(key) is not str:
                    raise WireError("non-string map key requires typed entry schema")
                if key in obj:
                    raise WireError("duplicate map key")
                obj[key] = read(depth + 1)
            return obj
        return [read(depth + 1) for _ in range(n)]

    def read(depth):
        if depth > MAX_DEPTH:
            raise WireError("nesting exceeds depth budget")
        b = number(1)
        if b <= 0x7f:
            return b
        if b >= 0xe0:
            return b - 256
        if 0x80 <= b <= 0x8f:
            return sequence(b & 0x0f, True, depth)
        if 0x90 <= b <= 0x9f:
            return sequence(b & 0x0f, False, depth)
        if 0xa0 <= b <= 0xbf:
            return text(b & 0x1f)
        if b == 0xc0:
            return None
        if b == 0xc2:
            return False
        if b == 0xc3:
            return True
        if b in (0xc4, 0xc5, 0xc6):
            return take(number({0xc4:1, 0xc5:2, 0xc6:4}[b]))
        if b in (0xca, 0xcb):
            v = struct.unpack(">f" if b == 0xca else ">d", take(4 if b == 0xca else 8))[0]
            if not (-float("inf") < v < float("inf")):
                raise WireError("nonfinite float")
            return v
        if b in (0xcc, 0xcd, 0xce, 0xcf):
            return number({0xcc:1, 0xcd:2, 0xce:4, 0xcf:8}[b])
        if b in (0xd0, 0xd1, 0xd2, 0xd3):
            return number({0xd0:1, 0xd1:2, 0xd2:4, 0xd3:8}[b], True)
        if b in (0xd9, 0xda, 0xdb):
            return text(number({0xd9:1, 0xda:2, 0xdb:4}[b]))
        if b in (0xdc, 0xdd):
            return sequence(number(2 if b == 0xdc else 4), False, depth)
        if b in (0xde, 0xdf):
            return sequence(number(2 if b == 0xde else 4), True, depth)
        raise WireError("unsupported or reserved MessagePack opcode")

    value = read(0)
    if i != len(raw):
        raise WireError("trailing MessagePack data")
    return value


def encode_atto_binary(value, schema_id):
    """Candidate new ATB1 envelope, not the existing Result/Option wire ABI."""
    if type(schema_id) is not bytes or len(schema_id) != 32 or not any(schema_id):
        raise WireError("expected nonzero 32-byte schema ID")
    payload = encode_msgpack(value)
    return _MAGIC + b"\x01\x00\x00\x00" + schema_id + struct.pack(">I", len(payload)) + payload


def decode_atto_binary(raw, expected_schema_id):
    if type(raw) is not bytes or len(raw) < _HEADER or len(raw) > _HEADER + MAX_BYTES:
        raise WireError("invalid or oversized ATB1 frame")
    if type(expected_schema_id) is not bytes or len(expected_schema_id) != 32 or not any(expected_schema_id):
        raise WireError("expected nonzero 32-byte schema ID")
    if raw[:4] != _MAGIC or raw[4:8] != b"\x01\x00\x00\x00":
        raise WireError("unknown frame magic, version or flags")
    if not hmac.compare_digest(raw[8:40], expected_schema_id):
        raise WireError("schema ID mismatch")
    n = int.from_bytes(raw[40:44], "big")
    if n > MAX_BYTES or n != len(raw) - _HEADER:
        raise WireError("invalid payload length or trailing bytes")
    payload = raw[_HEADER:]
    value = decode_msgpack(payload)
    if encode_msgpack(value) != payload:
        raise WireError("noncanonical ATB1 MessagePack payload")
    return value
