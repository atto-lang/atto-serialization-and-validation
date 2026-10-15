#!/usr/bin/env python3
"""Mutation tests for the two separate interface contracts (not compiler tests)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from transport_contract_audit import EXPECTED, validate_interface_source


def valid() -> str:
    return "\n".join(
        "pub define interface " + name + "<T> as\n  "
        + "\n  ".join(methods) + "\nend"
        for name, methods in EXPECTED.items()
    )


class TransportContractAuditTest(unittest.TestCase):
    def test_two_distinct_contracts(self):
        self.assertEqual([], validate_interface_source(valid()))

    def test_encoded_borrow_must_be_explicit(self):
        self.assertIn("NetworkCodec", " ".join(validate_interface_source(
            valid().replace("borrow T value", "T value", 1))))

    def test_decoded_payload_must_be_owned(self):
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(
            valid().replace(
                "fnc decode_actor_message(borrow List<int> bytes) => Result<T, DecodeError>;",
                "fnc decode_actor_message(borrow List<int> bytes) => Result<borrow T, DecodeError>;"
            ))))

    def test_no_cross_domain_method_substitution(self):
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(
            valid().replace("encode_actor_message", "encode_network"))))

    def test_no_control_codec_instead_of_actor_data(self):
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(
            valid().replace("ActorMessageCodec", "SupervisorControlCodec"))))

    def test_redeclaration_is_rejected(self):
        text = valid() + "\n" + "pub define class NetworkCodec as\nend\n"
        self.assertTrue(validate_interface_source(text))

    def test_comments_cannot_forge_missing_interface(self):
        text = valid().replace("pub define interface NetworkCodec<T>", "// pub define interface NetworkCodec<T>")
        self.assertTrue(validate_interface_source(text))

    def test_extra_optional_dispatch_is_rejected(self):
        text = valid().replace(
            "fnc message_schema_id() => String;",
            "fnc message_schema_id() => String;\n  fnc allow_control() => bool;")
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(text)))


if __name__ == "__main__":
    unittest.main()
