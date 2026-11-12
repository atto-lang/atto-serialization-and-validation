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

## Checked dynamic wire-graph preflight (experimental)

Directly constructed `Value` trees bypass all JSON/CBOR/MessagePack decoder
limits and well-formedness checks. Before returning `Ok(Value)`, the two
dynamic checked validators now inspect the **entire active-kind graph** using
an iterative, bounded queue: at most 65,536 visited nodes, at most 64 nested
container edges, and at most 16,777,216 inspected payload/key bytes.
Oversized or invalid trees return a payload-free `invalid_value_tree`
`ValidationReport` and never return `Ok`.

The walk checks wire kinds (0..8), numeric JSON lexemes, UTF-8 strings and
object keys, **0..255 octets for raw binary and extension payloads**, duplicate
object keys at every depth, child arrays, object values, and both map keys
and values. Every object, including nested open-schema objects, must stay
within a **1 MiB aggregate key-byte limit**. General maps also reject
semantically duplicate keys, including deeply equal reordered object keys.
After the full wire graph is checked, the map-key equality pass uses
hash buckets with a **256-comparison operation-wide limit**, rejecting
collision-heavy values instead of performing unbounded deep equality.
Crucially, every value entering hash/deep-equality is separately bounded to
**64 expanded nodes, 32 container edges, and 4,096 payload/key bytes**.
The same eligibility guard applies to the report-only `unique_items`
validator, which admits at most **4,096 array items** and has its own shared
**256-candidate-comparison limit**. Inputs outside these intentionally
conservative equality domains fail closed with a work-limit diagnostic
instead of being silently treated as distinct.

Schema validation, object-key duplicate detection, required-field lookup,
and unknown-field lookup also use a shared **256 failed same-hash text-key
comparison budget** per validation operation; checked tree preflight uses
one budget across the whole graph. Successful key matches do not burn this
collision allowance, so ordinary wide objects (including 300-field fixtures)
remain valid. Exhaustion is a failure, never treated as a missing field or a
successful unique-key check.

It does **not** recursively apply an arbitrary
object or element schema, authenticate an authoring authority or freeze the
returned mutable `Value`. Higher-level typed schema validation and
immutable compiler-minted `Valid<T, SchemaId>` remain separate release gates.
The quota is deliberately enforced on the direct checked APIs, not claimed
for all lower-level report-only validators or all decoder paths. A caller must
not rely on report-only validation as an authoritative trust boundary.

The preflight does not preserve/prove graph identity, schema freshness or
alias stability. Revalidate on later trust boundaries until ownership and
schema provenance are enforced by the compiler/loader. No native compiler
test result is implied merely by the presence of guest-source regressions.

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


## Object-schema builder ownership

`object_schema(fields, reject_unknown_fields)` snapshots the caller-supplied
field collection and each field's UTF-8 key-byte list. Mutating the original
builder list or key buffers afterward must not change which field the schema
requires. The constructor checks field count, **65,536 bytes per key**, and
**1,048,576 total key bytes** before copying. The same aggregate budget is
enforced independently when validating directly constructed `ObjectSchema`
instances or untrusted dynamic object `Value` keys; bypassing the builder
cannot bypass the quota. Checks use subtraction from the remaining allowance
to avoid overflow. Over-limit builder input yields an invalid-schema sentinel;
it must never become an empty, weakened, or partially copied usable schema.

This is builder-input detachment, **not** an immutable schema or a trusted
validation proof. `ObjectSchema.fields` and its nested lists remain publicly
mutable, and the dynamic `Result<Value, ValidationReport>` still contains a
mutable `Value`. Do not share mutable schemas between actors or cache
validation successes as `Valid<T, SchemaId>`. Compiler-backed ownership,
deep freezing, nominal authority, and complete wire-graph verification are
separate release gates.

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
`validate_value_checked` API returns an ordinary dynamic `Value` after
checking, not a substitute for the reserved proof type.
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
