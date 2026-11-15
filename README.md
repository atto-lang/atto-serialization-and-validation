# atto-serialization-and-validation

Serialization, schema validation, and interoperable data formats for AttoLang.

## Purpose

Encoding/decoding contracts, schema evolution, validation, and cross-backend wire compatibility.

## API and contract expectations

- Expose general-purpose interfaces that are usable without an Atto commercial account.
- Document data/ownership semantics, error behavior, and supported runtimes.
- Keep protocol and serialization details portable across relevant AttoLang backends.

## Development and testing

Use checked-in implementation and CI for actual commands. Add behavior-focused tests, including negative cases and lifecycle edge cases; distinguish local unit tests from executed multi-backend conformance.

## Project status

This describes intended scope, not a verified feature matrix. Consult the source tree and exact-commit CI results for implementation status.

## Ecosystem

- [AttoLang](https://github.com/atto-lang)
- [Language specification](https://github.com/atto-lang/atto-spec)
- [Cross-platform conformance](https://github.com/atto-lang/atto-platform-conformance)

## Repository ownership and contribution routing

**Belongs here:** Portable JSON/wire codecs, schema validation, strict parse/emit contracts and versioned data formats.

**Belongs elsewhere:** Language data types in atto-spec; RPC transport in atto-rpc; server applications in example repos.

**Security and portability boundary:** Bound recursion/size, reject malformed or ambiguous inputs, enforce explicit discriminants and ownership; don't deserialize host capabilities or closures.

AttoLang is the public name; preserve existing Oreslang/.ores compatibility and use canonical .atto for new portable library source. This README defines scope, not feature readiness.

**Acceptance:** name exact source SHA/tree, runtime/toolchain, positive and negative checks and execution receipt before claiming support. Host-only tests and empty CI steps do not prove guest execution.

## Tagged Option/Result JSON (host reference prototype)

`reference/tagged_json.py` now enforces an explicit `kind` (`option` or `result`) and `variant` (`none/some` or `ok/err`) with strict field sets. Errors require bounded snake_case codes and human messages. The reference limits UTF-8 JSON to 64 KiB, nesting to 32 levels, rejects duplicate keys and nonfinite numbers, and sorts keys when encoding. Run `python3 -m unittest discover -s tests -v` for host-only tests.

This is **not** the canonical Atto binary Result/Option wire format, not a general codec, and not proof that any Atto compiler emits these JSON tags. A future versioned wire contract needs cross-backend guest round trips and byte-for-byte evidence.
