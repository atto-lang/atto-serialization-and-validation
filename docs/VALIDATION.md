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

## Typestate

The library should support a validated wrapper concept so APIs can express that validation has already happened:

```text
Unvalidated<T> -> validate() -> Result<Valid<T>, ValidationReport>
```

This mirrors the useful Rust/Garde pattern and avoids accidental double validation at trust boundaries.

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
- `ValidValue` wrappers through `validate_value_checked` and `validate_object_checked`.

Error reports are bounded by `ValidationOptions.max_errors`, and validation errors retain logical path segments without retaining the entire untrusted input payload.

Pattern/regex engines, custom validator registries, cross-field rules, and compiler-generated nested typed validators remain integration work. They should lower into the same `ValidationReport` model instead of creating format-specific validation systems.

## Validation hardening and trust boundaries (October 2026)

- Invalid rule definitions are errors, not data that can be ignored. A string
  `min_length=4, max_length=3` is rejected even if the inspected value has
  five code points; array min/max contradictions and equal exclusive numeric
  endpoints are rejected too. These are **runtime Zorde rule checks**, not yet
  proof that the source compiler admits `type X = string with ...` syntax.
- Numeric range validation uses an approximate floating-point conversion.
  Non-finite constraints and conversion outcomes are rejected, and a nonzero
  decimal cannot silently underflow to zero. Precision-sensitive decimal
  boundaries still require an exact decimal comparison implementation before
  claims of fully correct JSON Schema `minimum` / `maximum` parity. For
  now, do not treat dynamic floating-point range validation as an exact-decimal
  verifier for finance or cryptographic amount admission.
- `is_url` is deliberately a **restricted, ASCII-only HTTP(S) shape check**.
  It rejects an empty authority, an empty host before `:`, dangling `:`,
  userinfo (`@`), and backslash authority ambiguity. It is not a complete
  URL parser or an RFC 3986 / WHATWG normalizer. Security decisions must
  separately canonicalize addresses, resolve DNS, and enforce destination
  allowlists; URL shape validation alone is not SSRF protection.
- Core `Value` remains transport-neutral data; this library's validation
  and typed wrappers live above the freestanding compiler core. Generated
  validators should compile admitted schemas into bounded, reflection-free
  checks. No schema or WIT declaration grants transport or FFI authority.

Source regression cases live in `tests/validation_test.ores`. The project
requires a real Oreslang CLI/runtime for `scripts/test.sh`; a source diff or
static audit is not evidence of guest execution.
