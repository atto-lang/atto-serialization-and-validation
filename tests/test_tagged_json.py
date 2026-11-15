import unittest
from reference.tagged_json import TaggedCodecError, decode_tagged, encode_tagged


class TaggedCodecTest(unittest.TestCase):
    def test_round_trips(self):
        for value in [
            {"kind": "option", "variant": "none"},
            {"kind": "option", "variant": "some", "value": None},
            {"kind": "result", "variant": "ok", "value": {"arr": [1, True]}},
            {"kind": "result", "variant": "err",
             "error": {"code": "not_found", "message": "missing"}},
        ]:
            self.assertEqual(decode_tagged(encode_tagged(value)), value)

    def test_reject_malformed_tags(self):
        for value in [
            {"kind": "result", "variant": "none"},
            {"kind": "option", "variant": "none", "value": 1},
            {"kind": "result", "variant": "ok", "value": float("nan")},
            {"kind": "result", "variant": "err", "error": {"code": "BAD", "message": ""}},
        ]:
            with self.assertRaises(TaggedCodecError):
                encode_tagged(value)

    def test_reject_ambiguous_json_and_depth(self):
        for raw in [b'{"kind":"option","kind":"option","variant":"none"}',
                    b'{"kind":"result","variant":"ok","value":Infinity}',
                    b' ' * 65537]:
            with self.assertRaises(TaggedCodecError):
                decode_tagged(raw)
        too_deep = 0
        for _ in range(34):
            too_deep = [too_deep]
        with self.assertRaises(TaggedCodecError):
            encode_tagged({"kind": "result", "variant": "ok", "value": too_deep})


if __name__ == "__main__":
    unittest.main()
