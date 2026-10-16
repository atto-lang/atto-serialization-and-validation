#!/usr/bin/env python3
"""Mutation tests for the two separate interface contracts (not compiler tests)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from transport_contract_audit import EXPECTED, validate_interface_source


def valid() -> str:
    return "\n".join(
        "pub define interface " + name + ("<T, F>" if name == "NetworkCodec" else "<T>") + " as\n  "
        + "\n  ".join(methods) + "\nend"
        for name, methods in EXPECTED.items()
    )


class TransportContractAuditTest(unittest.TestCase):
    def test_two_distinct_contracts(self):
        self.assertEqual([], validate_interface_source(valid()))

    def test_network_format_parameter_is_mandatory(self):
        self.assertIn("NetworkCodec", " ".join(validate_interface_source(
            valid().replace("NetworkCodec<T, F>", "NetworkCodec<T>"))))

    def test_actor_codec_cannot_acquire_network_format_parameter(self):
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(
            valid().replace("ActorMessageCodec<T>", "ActorMessageCodec<T, F>"))))

    def test_network_format_must_not_be_dynamic(self):
        self.assertIn("NetworkCodec", " ".join(validate_interface_source(
            valid().replace("NetworkCodec<T, F>", "NetworkCodec<T, DynamicFormat>"))))

    def test_compiler_ownership_proofs_are_not_guest_declarations(self):
        for name in ("Sendable", "Serializable"):
            for kind in ("interface", "trait", "class", "struct", "type"):
                declaration = (f"define {kind} {name}<T> as\nend"
                               if kind != "type" else f"type {name}<T> = int;")
                failure = validate_interface_source(valid() + "\n" + declaration)
                self.assertIn("reserved to the compiler", " ".join(failure),
                              (kind, name, failure))

    def test_schema_contract_cannot_hide_inside_foreign_module(self):
        forged = "define module TransportShim as\n" + valid() + "\nend\n"
        self.assertIn("unrelated declarations", " ".join(
            validate_interface_source(forged)))

    def test_proof_spoofing_in_sibling_source_is_rejected(self):
        import tempfile
        from transport_contract_audit import audit_transport_contracts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src/schema").mkdir(parents=True)
            (root / "src/guest").mkdir()
            (root / "src/schema/transport_interfaces.ores").write_text(valid())
            (root / "src/guest/forged.ores").write_text(
                "pub define trait Sendable<T> as\nend\n")
            failures = audit_transport_contracts(root)
            self.assertIn("reserved to the compiler", " ".join(failures))

    def test_proof_names_inside_comments_are_not_declarations(self):
        decorated = valid() + "\n// define trait Sendable<T> as\n"
        self.assertEqual([], validate_interface_source(decorated))

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
        text = valid().replace("pub define interface NetworkCodec<T, F>", "// pub define interface NetworkCodec<T, F>")
        self.assertTrue(validate_interface_source(text))

    def test_multiline_comment_declarations_are_not_proof(self):
        self.assertTrue(validate_interface_source("/*\n" + valid() + "\n*/"))

    def test_backtick_is_ordinary_source_token(self):
        from transport_contract_audit import code_without_comments_or_strings
        assert code_without_comments_or_strings(chr(96)) == chr(96)

    def test_nested_comment_is_ignored(self):
        from transport_contract_audit import code_without_comments_or_strings
        comment = "/" + "*" + "outer " + "/" + "*" + "inner " + "*" + "/" + " body" + "*" + "/"
        self.assertFalse("outer" in code_without_comments_or_strings(comment))
        specimen = comment.replace(" body", "\n" + valid() + "\n")
        self.assertTrue(validate_interface_source(specimen))
        nested_twice = specimen.replace("outer ", "outer /* extra */ ")
        self.assertTrue(validate_interface_source(nested_twice))
        self.assertTrue(validate_interface_source(specimen[:-2]))
        self.assertEqual([], validate_interface_source(valid() + "\n" + comment))

    def test_partial_block_comment_cannot_forge_network_interface(self):
        self.assertTrue(validate_interface_source(
            "/*\n" + valid().split("pub define interface ActorMessageCodec")[0]
            + "*/\n" + "pub define interface ActorMessageCodec"
            + valid().split("pub define interface ActorMessageCodec")[1]))

    def test_control_codec_declaration_is_reserved(self):
        forged = valid() + "\npub define interface SupervisorControlCodec as\nend"
        self.assertIn("supervisor CONTROL", " ".join(validate_interface_source(forged)))

    def test_unterminated_block_comment_fails_closed(self):
        self.assertTrue(validate_interface_source(valid() + "\n/* broken"))

    def test_comment_markers_inside_quotes_not_lexed_as_code(self):
        from transport_contract_audit import code_without_comments_or_strings
        clean = code_without_comments_or_strings("val String sample = \"text // /* comment\";\n" + valid())
        self.assertEqual([], validate_interface_source(clean))

    def test_borrowed_codec_cannot_be_hidden_inside_block_comment(self):
        malformed = "/* fnc decode_network(borrow List<int> bytes) => Result<T, DecodeError>; */"
        self.assertTrue(validate_interface_source(valid().replace(
            "fnc decode_network(borrow List<int> bytes) => Result<T, DecodeError>;",
            malformed)))

    def test_control_codec_in_other_source_file_is_reserved(self):
        import tempfile
        from transport_contract_audit import audit_transport_contracts
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src/schema").mkdir(parents=True)
            (root / "src/example").mkdir()
            (root / "src/schema/transport_interfaces.ores").write_text(valid())
            (root / "src/example/rogue.ores").write_text(
                "pub define interface ControlCodec<T> as\nend\n")
            self.assertTrue(any("supervisor CONTROL" in e
                                for e in audit_transport_contracts(root)))

    def test_non_interface_shadow_types_cannot_claim_codec_names(self):
        for kind in ("class", "struct", "enum", "pattern", "module", "contract", "actor"):
            source = valid() + f"\ndefine {kind} NetworkCodec as\nend\n"
            self.assertTrue(validate_interface_source(source), kind)
            source = valid() + f"\ndefine {kind} ControlCodec as\nend\n"
            self.assertIn("supervisor CONTROL",
                          " ".join(validate_interface_source(source)), kind)

    def test_comment_wrapped_mutation_matrix_rejects_all_missing_declarations(self):
        # Deterministic adversarial lexical matrix; never rely on comments
        # as evidence of implemented interface signatures.
        source = valid()
        methods = (
            "pub define interface NetworkCodec<T, F>",
            "pub define interface ActorMessageCodec<T>",
            "fnc encode_network(borrow T value)",
            "fnc decode_actor_message(borrow List<int> bytes)",
        )
        for marker in methods:
            for start, end in (("/*", "*/"), ("//", "\n"), ("/*\n", "\n*/")):
                forged = source.replace(marker, start + marker + end)
                self.assertTrue(validate_interface_source(forged),
                                f"{marker}: {start!r}")

    def test_extra_optional_dispatch_is_rejected(self):
        text = valid().replace(
            "fnc encode_actor_message(borrow T value) => Result<List<int>, EncodeError>;",
            "fnc encode_actor_message(borrow T value) => Result<List<int>, EncodeError>;\n  fnc allow_control() => bool;")
        self.assertIn("ActorMessageCodec", " ".join(validate_interface_source(text)))


if __name__ == "__main__":
    unittest.main()
