# Zorde Static Contract IR v1 (S0)

**Status:** first runtime representation/closed-record gate. Draft; compiler execution and trusted admission are not yet proved.

## Existing building blocks

- `src/schema/metadata.ores` is reflection-free typed codec metadata with duplicate source/wire/field-number checks, bounded field/name counts and per-field `ValueRule` validation.
- `src/schema/object_codec.ores` decodes bounded JSON into metadata-projected source/wire objects and applies required-field, field-rule and unknown-field validation.
- `scripts/zorde-static-contract-ir.mjs` is build-time only. It canonicalizes JSON object keys, computes SHA-256 Contract IR IDs, binds TypeSpec/authored JSON Schema/generated JSON Schema digests to a TJSV report, and rejects unsupported JSON Schema keywords before the scalar registry plan is emitted.

## New S0 runtime shape

`src/schema/contract_ir.ores` introduces `StaticContractIR`:

- version (currently exactly 1);
- bounded printable schema identifier;
- canonical Contract IR SHA-256 identifier;
- TypeSpec source SHA-256;
- independently authored JSON Schema SHA-256;
- generated JSON Schema comparison SHA-256;
- typed `CodecTypeMeta` record metadata.

The validator checks digest syntax (64 lowercase hex bytes), the schema ID, metadata constraints and the v1 closed-record policy (`unknown_field_policy == reject`). `decode_static_contract_json_attempt` refuses invalid Contract IR **before reading payload bytes**, then delegates to the existing bounded decoder/validator.

## Trust boundary — do not overclaim

The native S0 module does **not** recompute SHA-256 or authenticate the producer. The digest values are content-addressed provenance emitted by the pinned build-time TJSV tool. Their presence and shape are not proof that a caller-supplied record came from the trusted build. Only a trusted loader/compile pipeline may issue the static metadata to an application.

This module does not:
- execute BAML or a provider/network call;
- parse JSON Schema at request time;
- resolve remote/dynamic references;
- interpret arbitrary JSON Schema vocabularies, unions, recursive refs or cross-field predicates;
- mint a nonforgeable `Valid<T,SchemaId>` witness;
- freeze aliased payload graphs or certify actor-safe sharing;
- establish Graal, LLVM, Wasm, Cranelift/Forge or NativeAOT runtime parity.

The existing #29 warning about forgeable `ValidValue` and mutable aliases remains in force. Do not use this data wrapper as a security capability.

## Tests

`tests/contract_ir_test.ores` adds guest candidates for:
- version and schema-ID bounds;
- malformed Contract IR, TypeSpec, authored-schema and generated-schema digests;
- ignore/preserve unknown-field policies refused;
- valid closed-record JSON;
- unknown-field and missing-required-field refusal;
- invalid Contract IR refused before payload decode.

The tests are registered in `tests/main.ores`. They are authored, not yet compiled/executed.

## Next acceptance

1. Pin the S0 Contract IR generator to the TJSV action/toolchain revision and emit the exact `StaticContractIR` data shape from independently authored TypeSpec + JSON Schema, with complete source provenance.
2. Make the production trusted loader verify the canonical IR ID and all source digests against the actually checked out peer authorities; do not treat the native syntax check as admission.
3. Add TypeSpec/JSON Schema semantic drift/mutant receipts and unsupported-keyword negatives.
4. Prove immutable/owned metadata snapshots and nonforgeable validation witnesses before assigning security semantics.
5. Run exact-source native guest tests on Java/Graal and LLVM first, then Wasm/.NET/Cranelift/Forge where supported.
