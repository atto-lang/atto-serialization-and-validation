# Protobuf wire hardening, Zorde, WIT, TypeSpec and JSON Schema

**Status:** proposed integration / native source tests authored, not executed. This
document is a review contract, not a claim of native, ABI or schema parity.

## Boundary separation

| Layer | Authority | What it can safely decide |
| --- | --- | --- |
| `src/protobuf/wire.ores` | bounded raw wire syntax | tag number (1..2^29-1), wire type 0/1/2/5, 10-byte uint64 varints, fixed width, payload limits and byte validity |
| `src/protobuf/zorde.ores` | caller-supplied report-only rules | field presence, per-tag wire type, repetition count, per-field payload budget, unknown-field policy |
| Compiler-generated typed codecs | **authenticated** schema registry/descriptor | field meaning, packed encoding, signed/zigzag semantics, nesting, oneof, defaults, required, enum membership and refinement constraints |
| TypeSpec + JSON Schema | two independently authored *JSON diagnostic envelope* descriptions | shape of `WireMessage {fields:[{fieldNumber,wireType,payload}]}` only |
| WIT | proposed Component Model shape only | operation/resource/transport types, never a proof of validation or a working implementation |

**Protobuf is not JSON.** No schema-less wire parser may invent field names or
trust a JSON Schema to prove a Protobuf message is semantically valid. TypeSpec
and JSON Schema constrain an **adapter's JSON envelope**, not the raw .proto
schema or the protobuf bytes. A typed .proto schema will need its own generated
descriptor alongside the compiled Zorde registry.

### Wire invariants and forward compatibility

- Tag zero and tag >536870911 reject. Wire groups (types 3/4) reject deliberately.
- Tags 19000..19999 are syntactically valid on the wire: preserve unknown fields.
  They must **not** appear in a generated descriptor or `ProtoZordeFieldRule`.
  The core helper `proto_descriptor_field_number_valid` enforces this separation.
- Decoder preserves exact raw bytes of type-0 varint **values** including uint64
  above signed int64. A varint with ten bytes and terminal byte >1 rejects.
  Nine-byte signed-int64 max must remain exact. The encoder normalizes field
  **key** and length-prefix varints; byte-for-byte preservation of nonminimal
  keys/lengths is NOT promised. For signatures, retain the original input bytes.
- `ProtoField.payload` for wire 1/5 is exact fixed-width little-endian content;
  for wire 2 it is the content **excluding** the length prefix; for wire 0 it
  holds raw varint bytes when decoded. `proto_varint` constructs a logical
  signed field with an initially empty raw payload.
- Raw constructors now snapshot the caller's `List<int>`; mutation of the
  caller's original buffer cannot change those constructed field bytes. **But**
  `ProtoField.payload`, `ProtoMessage.fields`, and all nested lists remain
  mutable values. `fields_with_number` now returns flat per-field snapshots
  with independent payload lists, but `ProtoMessage` may still be constructed
  with aliased mutable field lists. These copies are NOT transitive immutability.
  No cross-actor sharing/proof is authorized by this change.
- The encoder preflights payload byte validity and available output budget
  before appending payload bytes. A failed encode returns an error, no
  partially written byte list is returned.
- The Zorde report layer checks manually constructed messages against
  the full **canonical re-encoded** message byte budget, including field keys,
  optional length prefixes and varint bytes (even when a logical constructor's
  raw varint payload list is empty). A decoder can discard nonminimal key and
  length prefixes, so this number is **not evidence of original input size**:
  `decode_message` separately enforces the size of the actual input buffer.
  `encode_message` separately enforces its output budget while emitting bytes.
  Zorde's byte accounting is report-only and does not establish signature or
  authentication semantics.

### Zorde admission

`zorde_validate_protobuf_wire(message, rules, decodeOptions, rejectUnknown)`
returns a bounded `ValidationReport` (up to 64 issues; `truncated` on overflow)
with no typed value or security capability. Each `ProtoZordeFieldRule`
specifies field number, wire type, required, maximum occurrences, maximum
payload bytes. It rejects duplicate descriptors, implementation-reserved
descriptor tags, malformed field shapes and resource-policy inconsistencies.
Unknown wire fields may be preserved or rejected by the caller policy.

Rules are **not trusted merely because they exist**. A compiler-issued/loader-
authenticated descriptor identity, schema fingerprint and ownership-safe
projection are prerequisites for generating any future `Valid<T,SchemaId>`.
No report, primitive, raw bytes, JSON envelope or WIT record can forge that
type. Revalidation is required after mutation; use deep ownership/freeze for
actor or FFI transfers. Generated typed codecs must additionally enforce
`oneof`, repeated/packed, unknown enum/field, nested depth, and all
compiler-issued Zorde constraints before side effects.

### WIT proposal and JSON envelope

`wit/protobuf-wire.wit` proposes `decode(data, limits)` and
`encode(message, limits)` returning WIT `result<T,codec-error>`. Every
field's payload in WIT must contain **raw** bytes; a future adapter must turn
the native `proto_varint` logical value into raw varint bytes without signed
overflow. WIT uses `u32` field numbers/limits/offsets, `u8` wire types and
`list<u8>` for payloads. The adapter must explicitly reject invalid limits
and over-budget inputs at every entrypoint. No WIT export currently exists.

`contracts/protobuf/main.tsp` and
`contracts/protobuf/authored.schema.json` are independently written
peer-authority envelope contracts. Neither is generated from the other. The
envelope structurally permits syntactically legal reserved unknown field
numbers; descriptor rejection lives in trusted Zorde/typed integration.
JSON Schema draft 2020-12 and TypeSpec cannot express the complete varint
continuation rules, exact fixed-width content, lifetime, budget, provenance or
all Protobuf semantic constraints without additional validators.

### Evidence gates / conformance plan

1. `node scripts/test-protobuf-contracts.mjs` — static shape/ABI-drift
   tripwire with adversarial mutations. **Not** a TypeSpec/JSON Schema/WIT parser.
2. `.github/workflows/protobuf-contracts.yml` — source-local, exact-head
   syntax/roundtrip test with checksum-pinned wasm-tools plus negative WIT
   mutant; independent TypeSpec/JSON Schema differential comparison against
   valid/invalid fixtures and semantic negative schema mutant. Retain receipts
   bound to the exact SHA. **Only counts if jobs execute nonzero steps.**
3. `tests/protobuf_test.ores` and `tests/protobuf_zorde_test.ores` —
   exact signed max, raw uint64, malformed tags/lengths/varints, fixed-width,
   tag reservations, mutable aliases, output budgets, rule/unknown policies,
   missing/repeated/typed fields and report truncation. Run a **real Oreslang
   compiler** on approved source-local execution with exact source-tree and
   compiler SHAs and nonzero-step evidence. These tests are authored only.
4. P0 follow-up: golden differential vectors against a mature Protobuf
   implementation; property/fuzz mutations (invalid 10th byte, boundary tags,
   partial lengths, overflow, repeated/packed, noncanonical prefixes),
   channel framing/cancellation/backpressure, nested typed descriptor
   conformance, and exact Java/GraalVM/LLVM/.NET NativeAOT parity.
5. P0 before release: compiler-only `Valid<T,SchemaId>` authorization,
   recursive immutable/owned freeze and actor-boundary replay/TOCTOU tests,
   schema-registry authentication and denial tests. Keep unproven compiler
   bridge gates closed.

This private repository's GitHub Actions have previously failed to execute
jobs (zero recorded steps). This change does **not** transfer private source
to another organization or authorize a runner workaround. A source-free
synthetic carrier test cannot substitute for source-local native execution.
