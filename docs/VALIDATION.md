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


## Unified dynamic Value graph and uniqueness budgets

All dynamic validation entrypoints (`validate_value`, `validate_object`,
and their checked `Result` counterparts) perform **one source-level bounded
graph preflight before rule evaluation**, even for `any_rule()`, open objects,
and arrays with no declared `max_items`. This is a runtime-input guard, not a
compiler-issued proof of `Valid<T, SchemaId>`.

The graph walk is iterative, permits **65,536 visited Value nodes**, up to
**64 nested container edges**, and at most **16 MiB aggregate active
payload/key bytes**. It rejects unknown `Value.kind`, noncanonical JSON
number lexemes, malformed UTF-8 strings or keys, out-of-octet binary or
extension payloads, duplicate nested object keys, and over-budget trees.
Shared subgraphs count on each traversal, so aliases do not exempt work from
the budget. Refusal returns an `invalid_value_tree` report; neither a
report-only call nor a checked call returns success on a malformed graph.

Object schema admission additionally caps aggregate schema-key bytes at
16 MiB. Schema collision checks, nested object duplicate-key checks, and
object field lookups each have a **16,384 candidate / 1 MiB compared-byte**
budget for their respective operation. A lookup work refusal is
`schema_lookup_work_limit`; it cannot be treated as an absent optional
field. A schema admission failure remains `invalid_schema`.

For a `unique_items=true` array the validator additionally enforces:

- at most **4,096 elements**, including when schema `max_items` is unset;
- at most **16,384 outer hash-bucket candidate comparisons**;
- **1,000,000 estimated structural-work units** across item hashing and
  equality, charged *before* each operation, including active bytes, nodes,
  and pessimistic multipliers for nested object/map collision scans.

Failures are `unique_items_limit`, `unique_items_complexity_limit`,
or `unique_items_work_limit`, respectively. If
`ValidationOptions.max_errors=0`, `report.truncated=true` still makes
`report_is_valid` false. Errors never retain untrusted payload values.

**Boundaries:** these are conservative admission/fuel estimates, not actual
instruction-count, CPU, memory or cancellation enforcement. The native
`value_hash`/`value_deep_equal` routines are not instrumented at each
recursive operation, and runtime/compiler/actor stack quotas remain separate.
No proof of arbitrary sharing, cycles, TOCTOU freedom, compiler parity,
schema authority or immutable validated typestate follows from these guards.
They require compiled guest tests before release, including worst-case
collision mutants and exact backend evidence.

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
