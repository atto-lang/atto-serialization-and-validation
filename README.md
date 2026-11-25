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

## Actor DATA host references

See [bounded Rust reference models](docs/ACTOR_DATA_HOST_REFERENCES.md). These are host test artifacts, not production codecs or compiled AttoLang guest support.
