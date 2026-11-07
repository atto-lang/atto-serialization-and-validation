# Proposed portable actor-event contracts (v1)

This directory establishes a **proposed** triple-format contract, not proof of a working AttoLang compiler/runtime. Do not mark any backend as implemented based on this repository alone.

- **JSON Schema 2020-12** is the normative UTF-8 JSON validation boundary; its oneOf cases are closed, event-tagged, versioned, and disallow unknown fields.
- **TypeSpec** represents the same three cases with explicit tagged models and a union; compile it to check the IDL independently. This is not a published REST API.
- **WIT** represents the portable Wasm component boundary. JSON snake_case members become WIT kebab-case. WIT option<T> fields map to omitted JSON properties, never JSON null. WIT does not enforce the JSON event-dependent conditions, string length limits, or version literal: validate before dispatch.

payload_utf8 is text with a 65,536-character JSON Schema cap, **not** a promise of arbitrary binary data or a stable byte-length bound. error_code is a limited diagnostic code, never a stack trace. This proposed interface does not make a trust, ownership, or cross-tenant authorization guarantee.

Run python -m pip install -r tests/requirements.txt and python -m unittest discover -s tests -v for schema vectors and three-format drift checks. Run tsp compile contracts/v1/actor-envelope.tsp --no-emit with @typespec/compiler for independent TypeSpec parsing. The workflow also invokes wasm-tools for WIT parsing and component-IDL validation; its GitHub-hosted execution remains **not verified**. Real Wasm guest/host adapter compilation and execution remain **not executed**. Treat the schema, TypeSpec, and WIT files as **draft v1**, not a release-qualifying compatibility guarantee.

## Hardened draft ingress
The Python `contracts/strict_json.py` reference decoder rejects duplicate JSON members (including nested), malformed UTF-8, nonfinite/fractional numbers, invalid Unicode surrogates, and inputs over 1 MiB before applying JSON Schema. This is **not** a claim that any production AttoLang backend uses the decoder. The trailing-newline regex bypass on `error_code` is closed, and tests now lock down TypeSpec and WIT field types and optionality. These are targeted shape checks, not a complete AST-level equivalence proof. Independent review, backend adapters and version-to-version compatibility evidence remain required.
