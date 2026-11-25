import struct
import unittest

from reference.portable_wire import (
    MAX_BYTES, WireError, decode_atto_binary, decode_msgpack,
    encode_atto_binary, encode_msgpack, validate_external_names,
)

SCHEMA = b"\x03" * 32


class PortableWireTests(unittest.TestCase):
    def test_golden_messagepack(self):
        self.assertEqual(encode_msgpack({"a": 1}), bytes.fromhex("81 a1 61 01"))
        self.assertEqual(encode_msgpack(["x", True, 0]), bytes.fromhex("93 a1 78 c3 00"))
        self.assertEqual(encode_msgpack({"z": 1, "a": 2}), encode_msgpack({"a": 2, "z": 1}))

    def test_round_trip_boundaries(self):
        values = [None, True, False, 0, -1, -32, -33, 127, 128, 255, 256,
                  32768, 2**32, 2**64-1, -2**63, 1.25, -0.0,
                  "", "ñ😀", "content-type", b"\x00\xff", [], {},
                  [1, {"x-y": [True, None, b"a"]}], {"": "ok", "z": 2}]
        for value in values:
            with self.subTest(value=value):
                self.assertEqual(decode_msgpack(encode_msgpack(value)), value)

    def test_utf8_exact_and_bytes_distinct(self):
        self.assertNotEqual(encode_msgpack("hello"), encode_msgpack(b"hello"))
        self.assertEqual(decode_msgpack(b"\xa0"), "")
        self.assertEqual(decode_msgpack(b"\xc4\x00"), b"")

    def test_noncanonical_but_valid_msgpack_accepted(self):
        self.assertEqual(decode_msgpack(b"\xcc\x01"), 1)
        self.assertEqual(decode_msgpack(b"\xca" + struct.pack(">f", 1.5)), 1.5)

    def test_msgpack_negative(self):
        failures = [b"", b"\xc1", b"\xa1\xff", b"\x81\xa1a\x01\xa1a\x02",
                    b"\x82\xa1a\x01\xa1a\x02", b"\x81\x01\xc0",
                    b"\xd4\x01\x00", b"\xc7\x00\x01", b"\xc0\x00",
                    b"\xdb\xff\xff\xff\xff", b"\xcb" + struct.pack(">d", float("nan")),
                    b"\xcf\x01"]
        for raw in failures:
            with self.subTest(raw=raw), self.assertRaises(WireError):
                decode_msgpack(raw)

    def test_msgpack_encode_negative(self):
        cycle = []
        cycle.append(cycle)
        for value in [cycle, {1: "x"}, 2**64, -2**63 - 1, float("nan"),
                      float("inf"), "\ud800", tuple([1]), bytearray(b"a"),
                      "x" * MAX_BYTES]:
            with self.subTest(value=type(value)), self.assertRaises(WireError):
                encode_msgpack(value)

    def test_depth_and_total_entries(self):
        deep = 0
        for _ in range(33):
            deep = [deep]
        with self.assertRaises(WireError):
            encode_msgpack(deep)
        with self.assertRaises(WireError):
            decode_msgpack(b"\x91" * 33 + b"\xc0")
        with self.assertRaises(WireError):
            encode_msgpack([[] for _ in range(4097)])
        with self.assertRaises(WireError):
            decode_msgpack(b"\xdc\x10\x01" + b"\xc0" * 4097)

    def test_external_aliases(self):
        self.assertEqual(validate_external_names({"content_type": "content-type", "x": ""}),
                         {"content_type": "content-type", "x": ""})
        for names in [{"a": "same", "b": "same"}, {"bad-name": "ok"},
                      {"a": "\ud800"}, {"a": 42}, {1: "hi"}]:
            with self.assertRaises(WireError):
                validate_external_names(names)

    def test_atb1_golden_frame_and_round_trip(self):
        result = encode_atto_binary({"a": 1}, SCHEMA)
        self.assertEqual(result[:8], b"ATB1\x01\x00\x00\x00")
        self.assertEqual(result[8:40], SCHEMA)
        self.assertEqual(result[40:44], b"\x00\x00\x00\x04")
        self.assertEqual(result[44:], bytes.fromhex("81 a1 61 01"))
        self.assertEqual(decode_atto_binary(result, SCHEMA), {"a": 1})

    def test_atb1_negative(self):
        f = encode_atto_binary({"a": 1}, SCHEMA)
        failures = [b"", f + b"z", f[:-1], b"BAD!" + f[4:], f[:4] + b"\x02" + f[5:],
                    f[:5] + b"\x01" + f[6:], f[:6] + b"\x00\x01" + f[8:],
                    f[:40] + b"\xff\xff\xff\xff" + f[44:],
                    f[:44] + b"\xcc\x01", f[:44] + b"\x81\xa1a\xcc\x01"]
        for raw in failures:
            with self.subTest(raw=raw[:8]), self.assertRaises(WireError):
                decode_atto_binary(raw, SCHEMA)
        with self.assertRaises(WireError):
            decode_atto_binary(f, b"\x04" * 32)
        for schema in [b"", b"\x00" * 32, "a" * 32]:
            with self.assertRaises(WireError):
                encode_atto_binary(1, schema)


if __name__ == "__main__":
    unittest.main()
