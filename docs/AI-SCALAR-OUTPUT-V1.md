# AttoLang AI scalar-record output admission v1

Tracking: [platform-conformance #317](https://github.com/ores-truffle-oreslang/oreslang-platform-conformance/issues/317). This is a **limited precursor** to BAML-style typed AI functions, not the completed typed AI runtime.

## Scope

`src/schema/object_codec.ores` exports:
- `ai_scalar_decode_options(): DecodeOptions` — restricted profile.
- `decode_ai_scalar_record_attempt(input, options, metadata, validation_options): ObjectDecodeAttempt` — explicit status and diagnostics.
- `decode_ai_scalar_record(...): Result<DecodedObject, ObjectCodecError>` — ordinary diagnostic result wrapper.

Both methods return a **dynamic** `DecodedObject` projection, not a nominally typed record and not an unforgeable `Valid<T>` witness. A model returning valid JSON is not automatically a trusted provider or an authorized action. The adapter must not be reused as an authorization/typestate gate.

### Supported field kinds

Version 1 only permits non-null JSON booleans (kind 1), JSON-number lexemes (kind 2), and UTF-8 strings (kind 3), each with explicit static metadata. Required and optional-absent fields are supported. A present JSON `null` is not an absent field and is rejected by these scalar rules. JSON numbers remain validated wire lexemes, **not** inferred signed i64 / decimal / double values; no numeric narrowing or coercion is performed. The v1 API additionally **rejects numeric min/max ValueRule constraints**, since the generic range validator compares through floating-point values and would not prove exact i64/decimal boundaries.

Other kinds (including untyped `any`, byte arrays, nested objects, arrays, maps, and explicit null fields) are intentionally `ai_unsupported_kind` until there is a separately audited typed descriptor and recursive value validation. Open-schema modes (ignore/preserve unknown fields) are rejected with `ai_open_schema`. Strict unknown-field rejection is required.

### Limits

The product profile uses: depth <=16, input <=65,536 bytes, string <=16,384 bytes, number <=1,024 bytes, and array/map entries <=256. Schema metadata is limited to 256 fields **before** scanning user-controlled names; binary decode budget must equal zero. Invalid or wider decode configuration returns `ai_limits` before parsing. The underlying strict parser checks actual input sizes; a full 65,536-byte message is permitted and 65,537 is rejected with `input_limit`. The generic codec's larger budgets do not silently apply to this AI admission boundary.

Current design deliberately includes no LLM requests, retries, provider credentials, network egress, streaming partial publication, prompt interpolation, tenant permissions, or dynamic schema loading.

### Diagnostics and evidence

- Status 0: validated *dynamic* scalar record. Status 1: metadata/profile failure. Status 2: JSON/options failure. Status 3: field-validation/root-kind failure.
- Errors leave decoded output at `consumed=0` and never return a partial successful `DecodedObject`.
- Test source: `tests/ai_scalar_record_test.ores`, registered in `tests/main.ores`. Includes 8 baseline positive, 16 baseline negative, 10 strict profile fixtures, 4 extra boolean/optional-null fixtures, the 256/257-field boundary, zero-binary-budget refusal, exact-number range refusal, 2 Result checks, exact input-size ceiling, and diagnostic code checks.
- A separate host-side reference oracle for the 38 scalar/policy fixtures is **not** AttoLang compiler or guest evidence.
- The PR remains draft until exact-head runner-backed compiler execution, actual guest output, independent security review, and branch protections. An Actions run with `steps:null` is a **runner admission failure**, not a compiler test outcome.

## Next implementation gates

1. Complete the parent [PR #47](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/pull/47) duplicate/trailing-data trust-boundary proof and compile native `tests/main.ores` on the exact stacked head.
2. Introduce canonical typed schema IR and deterministic digest (schema/source provenance). Verify TypeSpec/JSON-Schema compatibility separately; do not equate shape parity with nominal-type proof.
3. Add compiler-generated direct scalar constructors with a nonforgeable witness, borrow/ownership fence, and exact JSON numeric representation; address [issue #29](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/issues/29).
4. Extend nested/enum/array/Option semantics only with explicit per-type recursive validation, cost bounds, and 24+ deterministic conformance cases.
5. Add asynchronous mock-provider test and streaming/cancellation under tenant-owned actor isolation, then real authorized AOT guest execution per backend.

### Private-source CI policy

The private tree stays within its original authorized organization until a destination-specific source transfer is independently approved, source snapshot and numeric repository identity are bound, and outstanding source-carrier incident holds are resolved. A mirror that omits secrets/tokens still copies source; it is not exempt from this gate. Source-free carrier canaries do not prove compiler-backed execution.
