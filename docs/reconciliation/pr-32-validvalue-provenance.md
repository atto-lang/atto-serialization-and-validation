# PR #32 — dynamic ValidValue is not an authorization proof

Historical PR head `12a3d8e4c6e512f7d2c1be25466c8b49f38ad521`; current main `62e123bfa0d1f7a26578aa93df8dc56a80efc216`.

All historical changed API and test paths (`docs/VALIDATION.md`, `src/json/validated.ores`, `src/validation/validation.ores`, `src/zorde/zorde.ores`, `tests/validation_test.ores` and `tests/zorde_test.ores`) already match current main **exactly by blob SHA**. Do not replay a weaker wrapper or reintroduce forgeable `ValidValue` evidence into compiled/checked APIs.

Semantic contract: dynamic validation produces a validation result, not an opaque compiler-minted witness, immutable freeze proof, owned transport capability or actor-domain permit. Checked values must eventually be bound to schema digest/version, ownership, actor domain/generation and immutable graph validation. Do not allow object construction/casting/FFI to spoof a witness.

**Gate:** verify schema authority, alias/mutation rejection, source compiler and native positive/negative regressions with executed exact-head evidence; resolve cross-backend parity separately. This merge commit records existing integration only, not complete proof-carrier implementation.