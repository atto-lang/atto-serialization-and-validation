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
The pinned TJSV remediation [PR #235](https://github.com/ORESoftware/typespec-json-schema-validator/pull/235)
merged on October 18, 2026, after successful exact-head CI. The workflow
now pins the immutable merged commit `a9aa7e0a258a0514b433178842288ab798b0947c`.
This is not a separately published semver release pin and **does not** qualify
this private Zorde workflow, which still fails before runner allocation.
Changing the pinned action commit requires independent review and consumer CI.

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

## Source-local WIT syntax verification (unexecuted)

The new [`zorde-wit-parse.yml`](../../.github/workflows/zorde-wit-parse.yml) workflow is intentionally separate from the TypeSpec/JSON Schema peer-authority test. It is triggered by WIT and upstream schema-authority changes, uses the official `wasm-tools@1.262.0` release with exact SHA-256 `655b8df67d54c3ef9b52fc237c4e94bbfbb5f5f43825245cb0940a7002a9ed65` (no mutable install graph), and requires positive WIT syntax/roundtrip plus rejection of a malformed WIT variant. It creates a SHA-bound **parser-only** receipt with `canonical_abi_implemented=false`, `native_oreslang_guest_executed=false`, `wit_backward_compatibility_verified=false` and `release_ready=false`. No claim of an implemented Component Model adapter follows merely from valid WIT syntax.

**Not executed yet:** source-org Actions continue to return jobs with zero recorded steps. Actual parser execution, downstream WIT compatibility checks, Dafny proofs and native runtime conformance are still required under [#18](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/issues/18). The TJSV negative-mutation smoke in funded [sagitta-infra #33](https://github.com/sagitta-stack/sagitta-infra/pull/33) did not use this private WIT file.

## 2026-10-18 synthetic WIT toolchain proof and allocation diagnostic

The checksum-pinned wasm-tools installation and test method (WIT-to-JSON, binary-WIT roundtrip, binary validation, byte-for-byte JSON comparison, negative syntax input) was independently **executed and passed** on a funded private runner using wholly synthetic public WIT: [sagitta-stack/sagitta-infra PR #36](https://github.com/sagitta-stack/sagitta-infra/pull/36), [run 39160832295](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39160832295), artifact `12147714689`. No private Oreslang source was transferred and no private Zorde WIT file was copied. The verified parser installation replaces the slower Cargo compilation, with an official-release archive SHA-256 check before extraction.

To isolate infrastructure failure, [`sourcefree-runner-admission-diagnostic.yml`](../../.github/workflows/sourcefree-runner-admission-diagnostic.yml) runs with `permissions: {}`, no checkout, and a single environment-only shell step. Its initial [push run 39161105908](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/actions/runs/39161105908) and [PR run 39161108624](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/actions/runs/39161108624) both failed with **zero executed job steps**. This points to job allocation/policy/billing eligibility rather than a TypeSpec/WIT compiler failure, but cannot independently determine which administrative constraint is responsible. The private Zorde workflow remains `NOT_EXECUTED`.

Contract parity now checks mutations of **both** independent schema authorities, requires semantic rejection reports and emits SHA-256-bound positive/negative source-local evidence only if actually run. None of these changes certifies native JVM, LLVM, Wasm, WIT ABI or formal proof execution.


## 2026-10-18 static WIT boundary and pinned formal proof follow-up

The conservative `scripts/audit-zorde-wit-crosswalk.mjs` refuses unknown
package versions, injected functions, extra worlds/exports, unexpected WIT
records, size overruns and changes to the mirrored independently authored
JSON Schema fields. Its current actual source and the private WIT/schema
inputs passed **14 isolated V8 checks** (1 positive + 13 adversarial cases),
not native compiler/runner-backed tests. The WIT workflow also binds SHA-256
digests for the authored WIT, authored JSON Schema, crosswalk source,
parser JSON, binary WIT and receipt to its exact candidate source head.
A crosswalk is a limited static projection check, **not** JSON-schema
equivalence or WIT ABI implementation certification.

A new source-local [Dafny reference verification workflow](../../.github/workflows/zorde-dafny-reference-proof.yml)
installs a **SHA-256-pinned official Dafny 4.11.0 release** from
`dafny-lang/dafny`. Its positive job must prove
`formal/ZordeParity.dfy`; its independent mutant must fail with an
actual postcondition-verification error, not an installation/parse error.
Successful verification would demonstrate only the reference lemmas:
it does not prove runtime Zorde or the actor/compiler implementation.

The real Dafny toolchain mechanism **did execute on a funded private runner**
against a wholly synthetic, source-free admission model in
[sagitta-infra PR #37](https://github.com/sagitta-stack/sagitta-infra/pull/37):
[runs 39163407514](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39163407514)
and [39163410766](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39163410766)
both passed. Their deliberate false postcondition reported
`5 verified, 1 error` and retained artifacts `12150105884` /
`12148898958`. No private actor/compiler/Zorde tree was exported.

**Source-local Zorde proof run 39163514675 still failed with zero executed
steps**. It must remain `NOT_EXECUTED` until organization runner eligibility
is restored; no source-free test can promote this PR to production or
compiler parity. Independent private-source export approval remains blocked
by incident #22/#77 and protected-main prerequisites.


## October 18: schema keyword narrowing hardening

The WIT-to-JSON-Schema static crosswalk now additionally requires exact
Draft 2020-12 schema metadata and **exact supported JSON Schema keyword
sets** for the projected `ErrorCode`, `ValidationIssue`, and
`ValidationReport` types. It refuses unreviewed sibling keywords such as
`const`, `pattern`, `format`, `minItems`, `allOf`, `anyOf`,
`$dynamicRef` and renamed `$id` values, even when the record fields
and their JSON Schema `type` declarations are unchanged. This prevents
unreviewed silent tightening or loosening at the proposed WIT boundary;
it does **not** supersede independent TypeSpec peer-authority checking.

The actual source checker and current private WIT/authored JSON Schema
passed **26/26 isolated V8 checks** (one baseline and 25 adversarial
refusals). The source-local WIT workflow now demands `checks=26`
and binds that count plus SHA-256 of the parsed/binary WIT and
authored projection files to its receipt. Source GitHub Actions remain
`NOT_EXECUTED`, not source-validated.

The official TJSV comparison engine independently rejected a **same-type
boolean `const:false`** mutation on a **synthetic, source-free**
contract in [sagitta-infra PR #33](https://github.com/sagitta-stack/sagitta-infra/pull/33):
[executed run 39164525291](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39164525291)
passed all 17 job steps and retained artifact `12149717740`, including
the clean positive and three separate negative semantic comparison reports.
The extra narrowing report had SHA-256
`dbcd809f7a4e3308e0b169c9e7271b7dd0e6b65b0b5089370f735a1c79f759cb`.
No private source was exported; this proves source-free TJSV engine
refusal, not actual Zorde runtime equivalence.


## 2026-10-18 post-merge upstream action qualification

After [TJSV PR #235](https://github.com/ORESoftware/typespec-json-schema-validator/pull/235) merged, the source-free funded peer-authority/mutation suite was run again with the **exact merge commit** `a9aa7e0a258a0514b433178842288ab798b0947c`, not a floating `main` or unreviewed tag. [Runner-backed PR run 39167799979](https://github.com/sagitta-stack/sagitta-infra/actions/runs/39167799979) passed all steps, including independently authored positive parity, deliberately divergent JSON Schema and TypeSpec contracts, and a narrowed boolean-const mutant. Retained evidence: artifact `12151022508`; [source-free test PR #33](https://github.com/sagitta-stack/sagitta-infra/pull/33). The exact merge commit's `action.yml` blob is the same as the earlier reviewed head; the funded run verifies the merge commit's dependency/code closure as exercised by those synthetic probes.

This is **not** private Zorde contract execution. The source organization still fails to allocate runners even for a zero-checkout diagnostic, so `NOT_EXECUTED` remains the correct native/source-private qualification status. There was no cross-org private source transfer.
