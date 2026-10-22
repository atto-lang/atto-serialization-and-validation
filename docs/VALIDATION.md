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

## Dynamic Value structural admission

Dynamic `validate_value` and `validate_object` now preflight the **entire**
caller-provided graph, even for `any_rule()` or unknown fields in an open
object schema. A bounded iterative walk refuses malformed nested UTF-8,
duplicate object keys, unknown wire kind tags, oversized collections and
depth/node/byte budget exhaustion before applying rules or hashing array items.
Limits currently are 64 edges of depth, 8,192 visited nodes, 4,096 items per
collection, and 1 MiB of cumulative string/byte/number/key payload; repeat
references consume quota repeatedly and cycles are therefore rejected.
These are deliberately fixed limits pending compiler-generated per-profile
budgets. Errors never echo hostile payload contents.

**Remaining boundaries:** this is *not* a deep freeze, schema authenticity
proof, canonical JSON/CBOR/MessagePack equivalence, universal byte grammar
check, or compiler-issued `Valid<T, SchemaId>`. The legacy untyped number
lexeme/byte representations and mutable aliases still require follow-on
admission work. No source-compiler or backend pass is asserted until exact-head
native tests execute.

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
- fail-fast or bounded error collection;
- plain `Result<Value, ValidationReport>` checked dynamic results; no typestate authority.

Error reports are bounded by `ValidationOptions.max_errors`, and validation errors retain logical path segments without retaining the entire untrusted input payload.

Pattern/regex engines, custom validator registries, cross-field rules, and compiler-generated nested typed validators remain integration work. They should lower into the same `ValidationReport` model instead of creating format-specific validation systems.
