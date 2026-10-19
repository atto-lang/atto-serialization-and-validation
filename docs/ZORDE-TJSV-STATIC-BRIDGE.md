# Zorde static scalar bridge: TypeSpec / JSON Schema → Contract IR → native specs

**Status: experimental build-time lowering, not a schema-admission service and not
native execution evidence.** This document supplements the phase-one
`compiled_registry.ores` native implementation on draft PR #20.

## Review/merge dependencies

- Zorde runtime facade: draft #14 (base of #20).
- Independently authored peer schema contracts, corpus and TJSV action:
  draft #16, including `contracts/zorde/scalars.tsp`,
  `contracts/zorde/scalars.schema.json`,
  `contracts/zorde/scalar-instances/` and the new source-org workflow
  `zorde-static-scalar-contract-ir.yml`.
- This branch adds `scripts/zorde-static-contract-ir.mjs` and
  `tests/source_free/test_zorde_contract_ir.mjs`.

**Merge/rebase order:** merge the separately reviewed core fixes first,
then #14 and #16 after actual qualified tests, and rebase #20. Do not merge
an unexecuted stacked proposal merely because the bridge compiles a sample
plan or because the upstream TJSV action ran on synthetic data.

## Trust model

The independently authored TypeSpec and Draft 2020-12 schemas are peer
authorities. The official TJSV compiler/emitter produces a comparison-only
JSON Schema witness, runs in-memory semantic/differential comparison, and
emits a versioned, **derived** Contract IR if its checks pass.

This bridge can read the TJSV report and Contract IR after that procedure.
The host caller must supply separately reviewed SHA-256 digests for the
**exact bytes** of both inputs (e.g., from a protected build provenance
manifest). The CLI refuses absent/mismatching SHA-256s. A digest which the
untrusted supplier calculated for its own forged receipt is *not* a trusted
attestation. TJSV must be pinned, executed and independently reviewed; the
bridge cannot authenticate the original TypeSpec issuer or repo commit.

The source-free lowering then rejects unsupported declarations, unreviewed
excluded/out-of-scope TypeSpec declarations, disabled/refused differential
evidence, inconsistent identities/digests, unexpected scalar schema keywords,
and widened constraints. Only **full, bounded** scalar declarations are
supported:

| Type | Schema support | Compiled descriptor |
| --- | --- | --- |
| Integer | integer with explicit safe int32 minimum + maximum, optional `format:int32` | kind=2, property=1, exact interval |
| String | Unicode scalar length with explicit 0..65,536 minLength + maxLength | kind=3, property=2, exact interval |

No arbitrary `$ref`, `anyOf`, union, enum, object, array, JSON Schema
conditional, regex, coercion, dynamic `where`, `multipleOf`, open bounds
or int64 JSON-number bound is silently truncated/ignored. Exclusions are
empty in this first schema projection. Full int64 refinements remain in
compiler-owned normalized descriptor work; do not round through JS Number.

Example **after** a separate exact-head TJSV run and source/provenance review:

```sh
node scripts/zorde-static-contract-ir.mjs \
  --ir=artifacts/zorde-scalars-contract-ir.json \
  --report=artifacts/zorde-scalars-parity.json \
  --ir-sha256='<reviewed SHA-256 of the exact IR file>' \
  --report-sha256='<reviewed SHA-256 of the exact report file>' \
  --out-dir=artifacts/zorde-generated
```

The output is write-once `artifacts/zorde-generated/registry-plan.json`
plus `registry-specs.ores`; preexisting outputs are **not overwritten**.
The generated Oreslang function returns a list of
`ZordeScalarRegistrySpec` using the existing
`ZordeScalarDescriptor` and the current `compiled_registry.ores`
kernel. Downstream Oreslang **must still compile and execute this guest
file** in the target backend. A generated file is not a proof that the
compiler understands these declarations or that a trusted registry exists.

The plan has `runtimeImplementationVerified:false`,
`actorSafeSharingVerified:false`, and `releaseReady:false`. A registry
instance compiled from public/forged descriptors is just a bounded
validator; it **cannot** grant a `Valid<T>` proof or authorize cross-actor
mutable sharing. A trusted loader/capability, immutable snapshot, exact
source/IR digest binding, and eventually compiler-assisted nominal
construction remain mandatory.

## Test and release gates

The **source-free** Node test suite uses explicit synthetic Contract IR
fixtures. It exercises positive deterministic lowering and negative
semantic mutation, narrowing, hostile IDs and forged generated plans:

```sh
node --test tests/source_free/test_zorde_contract_ir.mjs
```

The source-org workflow `zorde-static-ir-bridge.yml` is currently
unable to allocate a runner. An isolated V8 test execution of the actual
pure JavaScript functions passed **44/44** checks, including 10,000 deterministic scalar projection property probes; this was **not** the
Node CLI, TJSV compiler or Oreslang native compiler. Run all of those
source-locally with pinned toolchains before changing this status.

Further required release gates: actual private Zorde TypeSpec-to-JSON
witness comparison; real Contract IR emitted by TJSV; full codegen
compile and guest execution on Java/Graal + LLVM; property/differential
corpus; memory/ownership proof; WIT component/ABI adapter; hostile
fuzzing; explicit trust-root for the hash-bound receipt. The open source
export incidents forbid private code mirroring to an unapproved public
funded Actions carrier.
