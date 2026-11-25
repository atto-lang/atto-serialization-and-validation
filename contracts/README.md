# Actor-event contracts — draft v1

Three authored contract views are checked in: JSON Schema 2020-12 (normative JSON
ingress), TypeSpec (typed models), and WIT (WebAssembly component boundary).
These files do **not** certify a backend implementation or make a production
security guarantee.

## Rust-first verification

Run cargo +1.88.0 test --all-targets to execute schema and strict ingress
tests, negative vectors, and field-type projection checks. The CI additionally
compiles TypeSpec using tsp and parses WIT using wasm-tools.

The Rust reference decoder rejects duplicate JSON object keys (including
nested keys), nonintegral numeric tokens, nonfinite tokens, malformed UTF-8,
invalid Unicode surrogates, and wire values above 1 MiB. It validates the
decoded object against the schema. No downstream actor runtime or host adapter
is connected to this reference decoder yet.

The actor message payload_utf8 is text, not arbitrary binary. WIT optional
fields are encoded to JSON as absent fields, not JSON null; the discriminant
determines which fields are required. WIT itself cannot enforce version
literals, conditional field presence, max string lengths, or Error subclass
semantics: hosts must validate again.

## Release and compatibility boundaries

- message, failure, and shutdown are draft tagged variants. They are not
  proofs of actor panic/throw/raise execution semantics or supervisor control
  messages.
- Source, tenant, and actor IDs are untrusted input until runtime authentication
  and authorization are implemented.
- Structural drift tests are not a complete semantic comparison of three ASTs.
- Do not rename or remove v1 public fields without a new major contract version,
  released-v1 compatibility fixtures, and cross-backend consumer verification.
- An exact-head CI pass is not independent PR approval or runtime conformance.

All repository CI/test/validation scripting is Rust-first; changes should
preserve this policy and update Rust regression coverage.
