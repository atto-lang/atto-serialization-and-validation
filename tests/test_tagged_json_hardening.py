import unittest
from reference.tagged_json import TaggedCodecError, decode_tagged, encode_tagged


class TaggedJsonHardening(unittest.TestCase):
    def test_round_trip_nested_and_disjoint_variants(self):
        for v in [
            {"kind": "option", "variant": "none"},
            {"kind": "option", "variant": "some", "value": None},
            {"kind": "result", "variant": "ok", "value": {"ñ": [True, 5]}},
            {"kind": "result", "variant": "err", "error": {"code": "not_found", "message": "ñ"}},
        ]:
            with self.subTest(v=v):
                self.assertEqual(decode_tagged(encode_tagged(v)), v)

    def test_reject_escaped_surrogates(self):
        for raw in [
            br'{"kind":"result","variant":"ok","value":"\ud800"}',
            br'{"kind":"result","variant":"ok","value":{"\ud800":1}}',
            br'{"kind":"result","variant":"err","error":{"code":"bad","message":"\udfff"}}',
        ]:
            with self.subTest(raw=raw), self.assertRaises(TaggedCodecError):
                decode_tagged(raw)

    def test_reject_ambiguous_and_out_of_range_numbers(self):
        for raw in [
            b'{"kind":"result","variant":"ok","value":18446744073709551616}',
            b'{"kind":"result","variant":"ok","value":-9223372036854775809}',
            b'{"kind":"result","variant":"ok","value":1e999}',
            b'{"kind":"result","variant":"ok","value":1,"value":2}',
        ]:
            with self.subTest(raw=raw), self.assertRaises(TaggedCodecError):
                decode_tagged(raw)

    def test_reject_large_aggregate_and_cycles(self):
        v = {"kind": "result", "variant": "ok", "value": [0] * 4097}
        with self.assertRaises(TaggedCodecError):
            encode_tagged(v)
        cyc = []
        cyc.append(cyc)
        with self.assertRaises(TaggedCodecError):
            encode_tagged({"kind": "option", "variant": "some", "value": cyc})

    def test_error_message_utf8_budget_and_unknown_fields(self):
        with self.assertRaises(TaggedCodecError):
            encode_tagged({"kind": "result", "variant": "err", "error": {
                "code": "error", "message": "é" * 260}})
        with self.assertRaises(TaggedCodecError):
            encode_tagged({"kind": "option", "variant": "none", "value": None})


if __name__ == "__main__":
    unittest.main()
