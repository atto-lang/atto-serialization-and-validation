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

Run (from repository root, with Node 22.9+ and installed, reviewed dependency):
```sh
npx tjsv check \
  --typespec=contracts/zorde/main.tsp \
  --schema=contracts/zorde/authored.schema.json \
  --instances=contracts/zorde/instances \
  --report=artifacts/zorde-schema-parity.json
```

CI provides a pinned **direct** tool version but has no npm lockfile yet: it
must not be treated as a supply-chain-qualified release gate until its entire
resolution graph is locked and dependency/advisory policy is reviewed.

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
