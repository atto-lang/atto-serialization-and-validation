# Zorde: two contract authorities + optional WIT projection (pilot)

**Stage:** proposed schema/interface integration only; no production Zorde runtime
implementation, compiled adapter, formally verified implementation or WIT binding is
claimed by this branch.

- `main.tsp` is independently authored authority **A**.
- `authored.schema.json` is independently authored Draft 2020-12 authority **B**.
- `tjsv check` emits a third JSON Schema witness from TypeSpec, parses both schema
  lanes **in memory**, compares declaration semantics/structure and runs **both**
  validators on deterministic probes plus the checked-in `instances` corpus.
- Any drift, unsupported vocabulary, missing declaration or contradictory expected
  fixture stops admission. Generated witnesses and receipts are never authorities.
- `wit/zorde-validation.wit` is a **downstream Component Model interface proposal**,
  not a JSON Schema replacement and not evidence that an ABI implementation exists.

## Locked CI pilot — still unadmitted

The pilot workflow no longer resolves npm dependencies ad hoc. It invokes an
**exact SHA** of ORESoftware/typespec-json-schema-validator containing its
lockfile and production advisory audit action, using Node **22.18.0** or newer.
The candidate action is under review in
[ORESoftware/typespec-json-schema-validator PR #235](https://github.com/ORESoftware/typespec-json-schema-validator/pull/235);
its revision is **not** a reviewed release pin until that PR passes its exact-head
CI and is admitted. Updating the SHA after merge requires separate review.

The workflow first compares separately authored TypeSpec and Draft 2020-12
JSON Schema, then constructs a **temporary** divergent JSON Schema under
`artifacts/` (changing ValidationReport.schemaVersion from 1 to 2).
It requires the same checker to reject the mutant; the generated source is never
promoted to a third authority, and the original authored schema is unchanged.

No passing workflow has been observed for this candidate branch yet.
GitHub Actions zero-step failures must be recorded as NOT_EXECUTED, not as
schema parity success. Even successfully executed parity and mutation tests
would not certify native Zorde, WIT compatibility, Dafny proof or actor safety.


## Zorde runtime boundary

`core` remains OS-free, allocator-free by default, reflection-free and usable
without TypeSpec, Node, a JSON DOM, a WIT runtime, or `stdlib`.
`alloc` and `std/zorde` may opt into schema loading, JSON decoding, compiled
validators and diagnostic reports using explicit allocation/capability providers.

At build or startup (NOT per request) a trusted `stdlib.validation` adapter
may load a bounded, verified, content-addressed TypeSpec-emitted witness and
authored JSON Schema, compare them in memory, and **refuse to register a
validator** unless semantic/differential admission succeeds. The admissible
receipt must bind *both* digests, generator/compiler versions, policy and
source revision. An unsupported JSON Schema keyword is **refused**, never
silently ignored. A successful check for finite examples is not a proof of
semantic equivalence for every possible JSON value.

Production decode/validate uses a precompiled, bounded validator with an
approved schema identity; it **does not invoke npm/TypeSpec or recompare both
schemas on every payload**. Unknown schema IDs, unchecked remote `$ref`,
excessive depth/size/time, recursion cycles, changed schema bytes or missing
receipts fail closed. No actor/worker may mutate the registry behind another
actor's immutable validation view.

WIT adapter types map to `ValidationReport` and `ValidationIssue` but
JSON's exact `schemaVersion=1` and `valid iff issues.empty` invariants, as
well as max payload, UTF-8, backpressure, cancellation and resource limits
require explicit adapter tests/proofs; WIT syntax alone does not encode them.

## Existing Zorde implementation to integrate

Serialization draft PR #14 already proposes `src/zorde/zorde.ores` and the
future canonical `std/zorde` library, with compiler-owned normalized
refinement descriptors and exact signed-int64 parsing paths. This pilot must
*reuse* those rules rather than start a second JSON-specific validation engine.
Its `ValidValue` facade does not currently prove deep immutability or alias
stability: a mutable Value tree must be frozen/copied with ownership proof or
revalidated at the next trust boundary. An external schema cannot authorize
unsafe actor cross-heap sharing. Do not lower exact signed-int64 rules
through the generic floating-number validation path, which may round values.

## Evidence levels

1. Dual authority + finite differential corpus (this PR's intended CI).
2. WIT parser + canonical ABI + baseline compatibility and binding tests (TODO).
3. Formal proof of reference schema-admission/scheduler/state-machine rules
   and machine-readable receipts bound to operation IDs (TODO).
4. Verified executable Zorde/native/JVM/Wasm implementation, hostile-input
   property/fuzz tests and cross-backend differential conformance (TODO).

`formal/ZordeParity.dfy` is a **small reference sketch**, not verified
evidence; it establishes only pointwise classification if checked with Dafny.
Never claim L3/L4 assurance merely because a proof file is present.

Cross-repo reference: ORESoftware/typespec-json-schema-validator and its
`docs/formal-verification.md`, `docs/wit-projection.md`; WIT parsing /
canonicalization should come from ORESoftware/ores-wit when that dependency
is accessible and pinned.

## 2026-10-18 funded source-free contract-check smoke

The Zorde pilot now pins TJSV candidate `b9f57de9b9c3d36437e75539fd8281f1f285f593` (full Ubuntu/macOS/Windows admission tests passed on that exact SHA). Its public TJSV action was **executed on a funded private runner using exclusively synthetic public test material**, without copying the private Oreslang actor/compiler tree. See [sagitta-stack/sagitta-infra PR #33](https://github.com/sagitta-stack/sagitta-infra/pull/33), [executed run 39155565618](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39155565618), and retained artifact `sourcefree-tjsv-positive-and-drift` (ID 12146133896). The job executed all steps, passed positive parity, and a deliberate independent schema mutation produced **exit 2** and `stopped_for_evaluation`. An independent receipt check required unexplained semantic findings, not just a failed step, and recorded SHA-256 of both receipts. It asserts `privateSourceTransferred=false`, `nativeOreslangCompilerExecuted=false` and `releaseReady=false`.

That is **real TJSV engine smoke coverage on synthetic input**, not a run of this private Zorde contract, WIT, Dafny or native `.ores` source. Zorde's own Actions still have no executed steps. Private source export remains blocked pending independently reviewed protected-main approval and incident #22/#77 clearance, regardless of available runner minutes.
