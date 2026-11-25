import unittest
from expanded_visibility import accepted


class ExpandedVisibilityTests(unittest.TestCase):
    def test_exact_private_targets(self):
        for fixture, line in (
            ("json-frame-visibility/private-state-import", b"error: JsonPrivateFrameState is not exported"),
            ("json-frame-visibility/private-state-mutation", b"error: JsonIncrementalDecoder.state is private"),
            ("sealed-schema/private-storage-import", b"error: cannot import PrivateObjectSchemaStorage"),
            ("sealed-schema/private-storage-access", b"error: private field SealedObjectSchema.storage"),
            ("json-frame-visibility/legacy-constructor", b"error: constructor JsonIncrementalDecoder argument count mismatch"),
            ("sealed-schema/direct-constructor", b"error: constructor SealedObjectSchema argument type mismatch"),
        ):
            with self.subTest(fixture=fixture):
                self.assertTrue(accepted(line, fixture))
                self.assertFalse(accepted(line + b"\nparse error", fixture))
                self.assertFalse(accepted(b"note: " + line, fixture))
                self.assertFalse(accepted(line + b"\x00", fixture))
                self.assertFalse(accepted(line + b"\n" + b"x" * 65536, fixture))

    def test_unrelated_and_spliced_denials(self):
        fixture = "sealed-schema/private-storage-access"
        for line in (b"error: Other.storage is private", b"error: state is private", b"SealedObjectSchema.storage\nerror: Other.storage is private", b"error: SealedObjectSchema.storage is public", b"error: OtherSealedObjectSchema.storage is private", b"error: SealedObjectSchema.storage is private but accepted", b"\xff"):
            with self.subTest(line=line):
                self.assertFalse(accepted(line, fixture))

    def test_constructor_binding(self):
        fixture = "sealed-schema/direct-constructor"
        self.assertFalse(accepted(b"error: constructor ObjectSchema argument type mismatch", fixture))
        self.assertFalse(accepted(b"error: SealedObjectSchema argument type mismatch", fixture))
        self.assertFalse(accepted(b"error: constructor SealedObjectSchema argument type mismatch", "unknown"))


if __name__ == "__main__":
    unittest.main()
