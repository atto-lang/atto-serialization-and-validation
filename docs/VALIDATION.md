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

## Rule consistency and restricted URL admission (October 2026)

- `value_rule_valid` already refuses impossible `min_length=4, max_length=3`
  and incompatible length/type combinations. Such a contradiction is invalid
  even if the inspected value would meet one of the two bounds. The runtime
  regression is in `tests/validation_test.ores`. **This does not implement**
  source-level `type X = string with length ...` declarations or compiler
  proofs of statically unsatisfiable types.
- `is_url` is deliberately a **restricted ASCII HTTP(S) shape check**.
  It bounds input at 8,192 bytes, rejects empty/malformed DNS-like labels,
  unexpected authority punctuation and userinfo, and admits numeric ports
  1–65,535 only. Bracketed IPv6-shaped literals require a colon and closing
  bracket but **do not yet receive full IPv6 address validation**. Raw IPv6
  without brackets is rejected. Backslash/control-byte ambiguities fail closed.
  This is not a complete RFC 3986 or WHATWG parser, grants no network
  authority, does no DNS/IP resolution, and is not SSRF protection. Consumers
  must separately canonicalize URLs and enforce destination allowlists and
  DNS/IP policies.
- The numeric-lexeme, non-finite conversion, noncanonical JSON number and
  bounded resource-admission fixes are being reviewed independently in
  [Zorde PR #21](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/pull/21).
  Do not mistake URL/schema shape tests for exact-decimal JSON Schema parity.

Real Oreslang guest execution of `scripts/test.sh` is required before this
candidate may be considered verified. No backend parity is claimed from a
source diff.
