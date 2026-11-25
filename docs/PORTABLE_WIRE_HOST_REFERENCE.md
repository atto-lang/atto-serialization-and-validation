# Tagged JSON and portable wire — host reference only

The Rust reference in `tools/portable_wire_reference.rs` preserves the intent of the original #4/#5 Python reference but follows the repository's Rust test policy. The original full snapshot remains at `e1acad0165a92e21c49b7c28e3b23c662288282b`, branch `recovery/portable-wire-python-full-snapshot-20261125`. This is not an AttoLang library, native guest implementation, ratified Result/Option ABI, runtime codec binding, or cross-backend proof.

- Tagged JSON requires exact option/result kind and variant field sets, bounded snake_case error codes and UTF-8 messages, strict full-consumption parsing, duplicate key refusal (including equivalent escapes), finite numbers and exact portable integer range. Bounded bytes, aggregate entries and depth are enforced. The owned Rust value representation does not express cyclic references.
- The deterministic MessagePack subset supports null, bool, signed/unsigned 64-bit integers, finite float, UTF-8 text, bytes, arrays and string-keyed maps. Other extensions and map-key types are unsupported. Noncanonical but valid supported MessagePack decodes; encoding chooses shortest integer and length headers, binary64 floats, and ordered map keys.
- The proposed ATB1 envelope binds version, flags, length, and a nonzero 32-byte schema identifier to that profile. Decoding requires canonical payload bytes. Schema matching is not peer authentication or schema-engine validation. This is not a ratified Atto binary format.
- Struct external-name metadata refuses invalid internal identifiers, duplicate external names and over-budget metadata.

Run `cargo +1.88.0 test --locked --test portable_wire_reference`. Native codecs must be implemented in canonical Atto source and tested by actual compilers against byte-identical fixtures and ownership/runtime constraints before being represented as supported guest features.
