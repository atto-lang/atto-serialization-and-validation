> **AI agent instructions (organization-wide):** Before working on code, issues, or pull requests, read and follow the [organization `AGENTS.md`](https://github.com/atto-lang/.github/blob/main/AGENTS.md), including its **merge-first stealth-development policy**. Honor any repository-local `AGENTS.md`, explicit holds, and GitHub-enforced rules.

# atto-serialization-and-validation

Serialization, schema validation, and interoperable data formats for AttoLang.

## Naming migration and language authority

This repository is maintained at [atto-lang/atto-serialization-and-validation](https://github.com/atto-lang/atto-serialization-and-validation); use `https://github.com/atto-lang/atto-serialization-and-validation.git` for Git remotes and canonical `.atto` source examples. The normative language contract is [atto-spec](https://github.com/atto-lang/atto-spec), with executable guest evidence tracked by [atto-platform-conformance](https://github.com/atto-lang/atto-platform-conformance). Follow current ownership, borrowing, effects, actor, and syntax rules there; do not infer implemented features from a README or unexecuted CI.

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

AttoLang is the canonical project name; new source, examples and documentation use `.atto` and the `atto-lang` repository organization. This README defines scope, not feature readiness.

**Acceptance:** name exact source SHA/tree, runtime/toolchain, positive and negative checks and execution receipt before claiming support. Host-only tests and empty CI steps do not prove guest execution.

## Legacy provenance and transfer verification

The historical predecessor was `ores-truffle-oreslang/oreslang-serialization-and-validation`. This repository is maintained in `atto-lang`; matching names or imported history do not establish preserved numeric repository identity or code parity. Preserve working legacy `.ores` inputs and historical package/CLI aliases until compiler-backed replacements and consumers are verified. New source follows canonical `.atto` contracts.

Portable structured serialization, typed validation and deterministic compatibility tests belong here; managed ingestion and production database administration do not. [atto-lang](https://github.com/atto-lang) maintains language, portable libraries and tooling; [atto-co](https://github.com/atto-co) separately governs commercial deployment, tenant infrastructure, identity, marketplace and billing. Any further original-ID transfer or commercial carve-out requires separate owner authorization and independent security review. This document authorizes no transfer or private-source mirror.

Before an authorized transfer, coordinate in [migration issue #26](https://github.com/ores-truffle-oreslang/.github/issues/26) and review the authorized runbook. Record source/target administrators, numeric ID, exact SHA/tree, history, visibility, access, rulesets, open PRs/issues, releases, packages, consumers, CI/billing/runners, secrets, deploy keys, apps, webhooks, Pages and rollback ownership. Verify unchanged ID/history/tree and restored controls afterward; execute exact-head tests before changing consumers. Zero-step jobs are NOT_EXECUTED; host-only and source-free checks do not certify compiled guest execution or production capabilities.

## Tagged Option/Result JSON (host reference prototype)

`reference/tagged_json.py` now enforces an explicit `kind` (`option` or `result`) and `variant` (`none/some` or `ok/err`) with strict field sets. Errors require bounded snake_case codes and human messages. The reference limits UTF-8 JSON to 64 KiB, nesting to 32 levels, rejects duplicate keys and nonfinite numbers, and sorts keys when encoding. Run `python3 -m unittest discover -s tests -v` for host-only tests.

This is **not** the canonical Atto binary Result/Option wire format, not a general codec, and not proof that any Atto compiler emits these JSON tags. A future versioned wire contract needs cross-backend guest round trips and byte-for-byte evidence.


## Host-only portable wire reference (candidate)

The stacked draft PR `#5` adds `reference/portable_wire.py` and focused negative/positive tests. `encode_msgpack` / `decode_msgpack` implement a **limited, deterministic MessagePack subset**: null, bool, signed/unsigned 64-bit integer, finite float, strict UTF-8 text, binary, arrays and string-keyed maps. Unsupported ext/typed non-string-key maps fail closed; the reference does **not** implement all of MessagePack or a typed Atto schema engine. `validate_external_names` rejects Struct internal/external field-name collisions.

`encode_atto_binary` / `decode_atto_binary` demonstrate a **proposed** `ATB1` 44-byte schema-bound frame around that MessagePack profile, with version, length and schema matching. This is a draft, not a ratified Atto Binary standard or the legacy `Result`/`Option` wire layout. See [atto-spec#73](https://github.com/atto-lang/atto-spec/pull/73).

The stacked PR also hardens `reference/tagged_json.py` against escaped unpaired surrogates, out-of-range integers, nonfinite numbers, excessive aggregate entries and cycles. It remains a host-only tagged `Option`/`Result` example.

Run from the repository root: `python3 -m unittest discover -s tests -v`. Local Python tests demonstrate host-reference behavior only. Required future proof: real Atto guest construction/decoding, schema registry, actor isolation, cross-backend golden vectors, fuzzing, exact source SHA and executed CI receipts. Do not count this prototype as guest support.

### Atto source ownership and native codec admission

A codec invoked as `encode(value)` must treat a nonprimitive `value`
as an implicit scoped readonly `borrow!`, equivalent to
`encode(borrow! value)`; serialization does not consume, share or mutate
the source object. Decode must allocate independently owned storage for
mutable nested values rather than rely on a shallow `copy!` of an
exclusive graph. `copy!!` requires a proven graph copier with memoized
cycles/aliases and failure rollback. Actor publication additionally needs
explicit authenticated `take!` or frozen/guarded `share!`; source
`pin!` and `drop!` rights cannot be reconstructed from wire bytes.

**Implementation gate:** the current Python files are reference-only and do
not count as an AttoLang library. Production codec implementation must be
native `.atto` source, with Rust supporting tests and real compiled guest
round trips. Do not promote host-only Python tests, speculative schema
metadata or unsupported graph-copy cases as backend conformance. Keep the
PR draft until the language implementation and independent ownership tests
exist.
