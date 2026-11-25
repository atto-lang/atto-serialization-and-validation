# Validation model

Validation is part of the serialization contract, but it is also independently callable.

## Generated validators

For known types, validation code is generated at build time. The normal decode path fuses validation into object construction so invalid payloads do not require a second reflective traversal.

A standalone validator remains available for values created by application code.

## Error shape

Each validation error should carry:

- machine-readable rule code;
- logical path (field/index path, not only a flat string);
- optional wire path;
- human-readable message;
- offending scalar summary when safe;
- source byte offset when validation occurs during decoding.

Do not retain entire untrusted payloads in error objects.

## Core rules

Presence:
- required
- forbidden
- required_if / forbidden_if

Strings/bytes:
- min_length
- max_length
- exact_length
- pattern
- ascii
- utf8
- email
- uuid
- url

Numbers:
- min
- max
- exclusive_min
- exclusive_max
- multiple_of
- finite

Collections:
- min_items
- max_items
- unique
- validate_nested

Cross-field:
- equals_field
- before/after
- one_of_fields
- custom validator

## Dynamic validation is not typestate

The checked dynamic helpers (`validate_value_checked`,
`validate_object_checked`, `zorde_parse*`) return ordinary mutable `Value`
results, not compiler-issued proof. `ValidatedDecodedValue.value` is a normal
value with a checked-at-decode-time result and a byte count. Any guest can
construct its own `Value` or metadata holder. Aliased nested storage can
change after validation. Neither success nor `report_is_valid` supplies
schema authentication, authorization, immutable ownership, nominal identity,
or cross-actor/FFI admission. Revalidate at every relevant use/trust boundary;
freeze/move storage separately before trusting its contents.

Previously the publicly constructible `ValidValue` wrapper misleadingly
resembled a proof; it has been removed. This intentionally breaks consumers
using `ValidValue` or `.valid.value`. Use `Value` and `.value` instead.

**Not implemented:** future authoritative `Valid<T, SchemaId>` must be
compiler-minted with inaccessible constructors, no forged casts/subclasses,
frozen/owned nested graph, schema digest/version/generation binding, and
lifetime-safe actor/FFI/hot-reload transitions. Dynamic results are not that.

## Aggregate object key budget

The root object validator independently caps schema and input object keys
at 4,096 fields, 65,536 bytes per key and 1,048,576 bytes in aggregate per
object. The aggregate length-only precheck precedes UTF-8 scans and hash
bucket construction. Publicly constructed schemas and Values are subject
to the same checks at validation. This is not a recursive graph budget,
schema immutability guarantee or a bound on collision-comparison work.
Native execution of these new boundary cases is required.

## Checked dynamic graph structural preflight

Checked value and object admission now walk the active payload graph before
schema-rule validation. Limits: 65,536 node occurrences, root depth zero with
maximum depth 64, 16 MiB cumulative active payload bytes, and the per-object
key limits above. Map keys and values count separately. Repeated references
are charged per occurrence; cycles cannot make the bounded walk infinite.
Unknown kinds, malformed numeric lexemes, invalid UTF-8, and non-octet raw
binary/extension payloads fail closed with a payload-free invalid_value_tree
report. Nested text-object and general-map duplicate keys are denied. General-map
key hashing and equality run only after the entire active graph passes
structural validation. A single 1,000,000-credit logical meter and 16,384
candidate ceiling span all general maps and all unique-array rules in
one validation operation; incomplete hashes or comparisons deny admission.
Conservative uniqueness estimates also accumulate across fields. Resource
refusals stop field traversal even with fail_fast disabled. This does not validate inactive slots, meter every schema lookup
or prove physical heap bounds, ownership or metadata authenticity. Native
tests remain required; exact cumulative structural-byte boundary tests
are pending.

## Dynamic encoder graph admission

JSON, MessagePack and CBOR dynamic encoders now run the same structural
preflight before constructing a writer or hashing general-map keys. Expanded
Value occurrences are limited to 65,536 and depth to the caller's valid
`EncodeOptions.max_depth` (maximum 128). Every occurrence and active string,
binary or extension payload contributes to a conservative minimum output
size. Object keys also charge their length plus one byte. The actual writer
still enforces exact output length and codec-specific format validity.
Numeric lexemes do not contribute their full textual length because binary
formats can compress them. This is a new compatibility limit for oversized
dynamic graphs; a graph_limit error is a resource refusal.

Cycles and shared subgraphs cannot expand the structural traversal beyond
these limits. This is not an identity-preserving graph codec, physical heap
bound, or complete bound on writer-side collision comparisons; recursive
hashing and equality in compatibility codec paths remain separate work.
The numbered Protobuf wire codec is not a dynamic Value encoder.
Native execution remains required. The isolated candidate programs under
`fixtures/graph-ownership/` exercise direct and mutual cycles and shared
subgraphs; they are not yet admitted to the default native suite until
compiler ownership behavior is established.

## Binary codec comparison fuel

Dynamic CBOR and MessagePack readers and writers allocate one logical
1,000,000-credit key-work budget per operation. Every structural key hash,
recursive semantic comparison and candidate probe consumes that budget.
An incomplete hash or comparison yields `key_work_limit`, regardless of
duplicate policy. Completed hashes are reused for bucket insertion. A
metered key can have at most 64 recursive levels relative to its own root;
this is a deliberate compatibility restriction independent of the
configured wire nesting depth (up to 128). Separate structural, allocation,
and JSON text-key work remain outside this meter. These are logical work
units, not a wall-clock or heap guarantee.

## JSON key work and terminal frame semantics

JSON readers and writers share the same 1,000,000-credit logical key meter
across all nested objects. Charge one credit per key hash, every hashed byte,
each candidate probe, and each compared byte. Reuse completed hashes; a
comparison refusal yields key_work_limit under every duplicate policy.

The incremental scanner does not semantically parse each chunk. It performs
one terminal semantic parse at a candidate boundary or EOF. Any parse
failure, including a candidate that turns out incomplete, invalid options,
invalid bytes or input-limit failure, terminates the decoder. Subsequent
feed/finish requests cannot reset fuel or retry the same frame. The
existing channel adapter inherits this behavior and still drains a
successful root through EOF to reject trailing data. This is a change
from retryable failed feed calls; create a new decoder for a new frame.
Native execution and compiler ownership checks remain required.

## Cumulative dynamic decoder materialization limits

JSON, CBOR and MessagePack use one operation-local logical materialization
budget: 65,536 node visits and 16 MiB of decoded payload credits. Keys count
separately from values, even when first/last-wins discards an occurrence.
JSON counts decoded string/key bytes and number lexemes before appending,
including full UTF-8 scalar widths. Binary payload lengths are charged
before copying; integer conversion reserves 20 lexical bytes per integer
conservatively. Declared binary child counts must fit the remaining node
budget before child decoding starts. JSON map-entry count now measures
wire occurrences rather than unique retained fields. These are new
compatibility limits in addition to DecodeOptions. No public constructor
arity changes. Budget denial aborts the parse and cannot publish a partial
result.

The limits do not measure actual allocator overhead, temporary containers,
input buffering, map bucket internals or backend-specific object sizes.
Native compilation and allocator instrumentation remain required. The native
suite contains exact/one-over cumulative node and duplicate occurrence
integration controls. Large 16 MiB payload boundary tests still require
an independently bounded test carrier; direct meter boundary assertions
are not a substitute for those integration tests.

## Incremental decoder private storage

Construct incremental decoders with new_incremental_decoder and interact
through feed, finish and received_bytes (a scalar count including trailing
whitespace). The mutable buffer, scan index, terminal flag and configuration
reference are stored in a non-exported nominal JsonPrivateFrameState; the
public decoder holds a non-public state field. Direct construction with
caller-owned buffers and direct state mutation are not supported APIs.
The channel adapter uses received_bytes instead of accessing the buffer.
Isolated positive and negative compiler fixtures are staged; actual
visibility and constructor enforcement remain native release gates.
This is encapsulation, not a claim of thread-safe mutable sharing.

## Schema builder ownership and explicit refusal

Use `object_schema_checked(fields, reject_unknown_fields)` when constructing
dynamic schemas. It checks the field count, per-key and aggregate key-byte
limits before allocating copies, detaches the field list and each key buffer,
and validates the detached schema. Invalid input returns a payload-free
`Err(ValidationReport)`. The old `object_schema` adapter preserves its
signature and returns an invalid UTF-8 sentinel on refusal, never an empty
permissive schema. The sentinel itself is mutable, not a sealed capability.

This is builder-input detachment, not deep freezing: the returned
`ObjectSchema.fields` and key lists can still be deliberately mutated.
Every validation rechecks schema validity. Callers must enforce actor-local
ownership and must not treat dynamic success as a compiler-minted proof.
Native execution and independent ownership review remain release gates.

## Sealed object policy

For an actor-local reusable policy use `seal_object_schema`, which returns
`Result<SealedObjectSchema, ValidationReport>` after bounded snapshot and
validation. Its nominal storage class and state field are non-public; the
only introspection API returns a scalar field count. Validate through
`validate_sealed_object`, `validate_sealed_object_checked` or strict
`decode_and_validate_sealed_object`. Legacy ObjectSchema remains mutable.
The report path builder copies key bytes so diagnostic mutation cannot
rewrite a sealed rule through an indirect alias. No raw policy getter or
unchecked sealing adapter exists. Native visibility, constructor denial,
ownership and mutation tests remain required. This is an encapsulated,
detached dynamic policy, not a frozen cross-actor capability or proof of
compiler-issued schema identity.

## Bounded diagnostic paths

The dynamic object validator retains at most 256 bytes of each diagnostic
key. Longer keys are omitted entirely, not truncated into invalid UTF-8.
PathSegment kind 3 means omitted oversized object key; kind 1 with an empty
key remains a genuine empty field name. Error codes and rejection semantics
are unchanged. With at most 4096 errors and the current single-segment
object paths, retained logical key payload is bounded by 1 MiB (not heap
bytes). Keys are still checked at their full admitted length; this is only
a diagnostic representation restriction. Consumers must handle kind 3.
Native regression assertions remain unexecuted.

## Schema-bound decoder policy

`decode_and_validate_object` is a **strict** JSON trust-boundary helper:
`duplicate_key_policy` must be `0` (reject), and
`allow_trailing_data` must be `false`. Any other configuration is refused
with a decode error before parsing. This avoids first/last-wins policies
silently discarding duplicate evidence, or a valid JSON prefix being mistaken
for admission of an entire untrusted message. General codec decode functions
may continue exposing explicit compatibility policies, but callers must not
use them as a Zorde security-boundary substitute. The return value remains
an ordinary mutable `Value`, not an opaque validation proof.

## Streaming validation

Rules should run as early as possible:

- reject a string once it exceeds max_length;
- reject an array/map once count exceeds max_items;
- mark required fields as seen while scanning;
- validate scalar ranges before storing them;
- defer only true cross-field rules until the enclosing object completes.

This prevents large invalid payloads from being fully materialized.


## Native validation library

`src/validation/validation.ores` now provides the runtime-independent rule/report layer used by generated validators and dynamic payload validation.

Implemented primitives include:

- required and forbidden object fields;
- expected wire kind;
- UTF-8 code-point min/max string length;
- byte min/max length;
- array min/max item count;
- unique-array validation;
- inclusive/exclusive numeric minimum and maximum;
- ASCII checks;
- canonical UUID shape checks;
- basic email shape checks;
- HTTP/HTTPS URL checks;
- reject-unknown-field object schemas;
- bounded input object member counts and key bytes, with UTF-8 key validation even in open schemas;
- fail-fast or bounded error collection;
- plain `Result<Value, ValidationReport>` checked dynamic results; no typestate authority.

Error reports are bounded by `ValidationOptions.max_errors`, and validation errors retain logical path segments without retaining the entire untrusted input payload.

Pattern/regex engines, custom validator registries, cross-field rules, and compiler-generated nested typed validators remain integration work. They should lower into the same `ValidationReport` model instead of creating format-specific validation systems.


## Oreslang `with` numeric refinement integration (staged exact-int kernel)

The compiler/shared contract normalizes all three spellings to the **same**
`zorde.scalar/v1` metadata and fingerprint:

```ores
declare type Code;
fnc identity(Code value): Code { return value; }
define type Code = int with min 0 and max 5;

// Equivalent constraint, not a separate schema:
define type EquivalentCode = int with value gte 0 and value lte 5;
```

`@Range(min=0,max=5)` and `Zorde.int().min(0).max(5)` also normalize to
the same interval predicate (but names/nominal identities remain compiler
type-system concerns). The shared JSON example lives at
`tests/fixtures/zorde-code-0-5.json`; it is byte-identical to
`oreslang-common-source/fixtures/refinement-v1/zorde-code-0-5.json`.
Inclusive min/max allow 0 and 5 and reject -1 and 6. Strictly positive
means `with value gt 0`, not `with min 0`. Arbitrary natural language
`int as positive integer` is not a validation rule and must not be parsed
as one.

`src/validation/refined_int.ores` now offers `validate_refined_int`,
`refined_int_schema_valid`, and `refined_int_accepts` as a bounded,
reflection-free, exact signed-int64 **report-only** validation kernel.
It uses `parse_int` over the number's original lexical bytes instead of
the existing general `number_rule` floating-point path; this distinction
matters above `2^53`. It rejects decimal, exponent and out-of-range
integer encodings, contradictory bounds, sorted/unique-exclusion violations,
and unsatisfiable intervals. The helper is included in both native test suites.

**Security and release boundary:** Dynamic rule arguments or a user-provided
schema fingerprint **never** mint compiler-authoritative
`Valid<T, SchemaId>` or an Oreslang refined `Code` value. Source-level
`with` parsing, static implication and checked-constructor codegen,
trusted schema-identity admission, generated decode-time dispatch, and
Java/LLVM executable parity are still pending. The existing
`validate_value_checked` / `ValidValue` path is an ordinary dynamic
validation report wrapper, not a substitute for the reserved proof type.
Do not silently lower a refined `int` to plain `int` or route exact bounds
through float-based `number_rule`; fail compilation instead until typed
end-to-end lowering is verified.

## String length and character-set refinements (v2; staged)

The canonical scalar string policies are:

```ores
define type Handle = string with min_length 2 and max_length 4
    and chars ascii_letters;
define type UpperCode = string with min_length 1 and chars ascii_uppercase;
define type LowerCode = string with min_length 1 and chars ascii_lowercase;
```

`min_length N` and `max_length N` normalize to `length gte N` and
`length lte N` without changing Unicode scalar counting. The character
policy `chars ascii_letters` permits only ASCII A-Z / a-z; `ascii_uppercase`
only A-Z; `ascii_lowercase` only a-z. Digits, punctuation, combining marks
and non-ASCII letters do not satisfy these policies. Empty strings satisfy
character policies vacuously; use `min_length 1` to forbid emptiness.

`src/validation/refined_string.ores` provides an exact UTF-8, report-only
validator. Character policy enum: 0 any, 1 ascii_letters, 2 ascii_uppercase,
3 ascii_lowercase, 4 Unicode uppercase, 5 Unicode lowercase, 6 Unicode Han.
Policies 0–3 and **6 (Unicode 18 Script=Han)** have report-only executable validation. Policy 6 uses all 25 exact Unicode 18.0 `Scripts.txt` Han ranges (103352 code points, including supplementary-plane extensions). Policies 4–5 (Unicode Uppercase/Lowercase) still reject **all** inputs with `unverified_unicode_policy` until certified Unicode **18.0.0** derived property tables and differential Java/LLVM tests exist. Do not substitute host
Unicode defaults or limited CJK block ranges for Script=Han. The exact data source is https://www.unicode.org/Public/18.0.0/ucd/Scripts.txt. Han-script
characters are not equivalent to the Chinese language.

Character policies use `zorde.scalar/v2`; length-only aliases continue
using unmodified `zorde.scalar/v1` identities. Version 2 includes policy
name and pinned Unicode table version in its canonical fingerprint.
The shared fixture
`tests/fixtures/zorde-english-letters-2-4-v2.json`
must exactly match the common-source counterpart. Native string tests are
included in both the core and full suites, covering UTF-8 validity,
length semantics (including emoji/combining marks), ASCII policy checks,
unknown/Unicode policy rejections and bounded errors.

The validator does not mint typed `Valid<T,S>`, dispatch automatically from
guest `with` syntax, or claim verified runtime/compiler acceptance.
Compiler-generated schema-identity admission and end-to-end Java/LLVM
refined type checking remain separate release gates.

## Bounded graph and uniqueness admission (2026-11-25 candidate)

Both report-only and checked dynamic validation enter the same occurrence-based
structural preflight; checked wrappers reuse the report result rather than
scanning the tree twice. Nested malformed values and duplicate object keys
cannot hide behind any_rule or open schemas. Structural denials return
invalid_value_tree (including some formerly root-specific errors; review
compatibility before release). This is a bounded expanded tree, not an identity
preserving graph codec. Cyclic inputs are refused by depth or occurrence limits;
shared acyclic subgraphs are charged for every expanded occurrence.

Unique-items now ports the metered primitives from #30. One operation-local
ValueWorkFuel is shared by all candidate hashes and comparisons, with a separate
conservative structural cost limit and collision-attempt limit. Incomplete
hash/equality is a validation error, never a successful hash or false equality.
Compatibility value_hash and value_deep_equal remain unchanged for existing
trusted callers; they are not certified untrusted-graph APIs. The meter is not
a wall-clock quota, and the current code does not mint authoritative Valid<T>.
Root/schema key lookup fuel, general-map duplicate semantics, true identity-aware
cycle diagnostics, stable graph snapshots, and cross-backend AOT tests remain gates.
Native guest execution remains NOT_EXECUTED until exact-head compiler evidence.

## Native JSON cursor ownership follow-up

Port the explicit mutable cursor and writer reborrows from legacy PR #33
without reverting the newer comparison, materialization or terminal-state
checks. Numeric lexeme append takes a mutable loan; integer UTF-8 byte
extraction uses shifts instead of floating division. Standalone cursor
smoke programs are staged for the independently approved native compiler.
This is a source repair candidate, not a verified ownership result.

## Restricted actor MessagePack admission

`decode_actor_bounded(input, options, max_logical_bytes, max_work)` ports
legacy #27's restrictive actor profile. Strict duplicate and trailing-data
policy, max depth 64, max wire 1 MiB, max logical budget 8 MiB and work
4 Mi credits. Precharge 128 base + 16 per wire byte, and 3 work per wire
byte; charge 128 logical bytes and 1 work credit per value visit. Declared
array children must fit both remaining credits before traversal. Maps and
extensions are refused at any depth. Generic decode remains unchanged and
both paths retain the newer global materialization and key-work limits.
These are logical units, not physical heap or CPU measurements; native
execution, actor mailbox integration and ownership review remain required.
