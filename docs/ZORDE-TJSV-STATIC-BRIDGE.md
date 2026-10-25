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
  --instances=contracts/zorde/scalar-instances \
  --out-dir=artifacts/zorde-generated
```

The output is write-once `artifacts/zorde-generated/registry-plan.json`,
`registry-specs.ores` and a restricted pilot `registry-smoke.ores`.
Only after all three files exist is `build-complete.json` issued with
individual SHA-256 hashes and `guestSmokeCompiled:false`,
`guestSmokeExecuted:false`, `releaseReady:false`. Existing output
directories are refused rather than overwritten.
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
pure JavaScript functions passed **73/73** isolated pure-function cases
(including 10,000 deterministic scalar projection probes) with simulated
noncryptographic hashing. These runs do **not** execute the Node CLI, genuine
SHA-256/file I/O, TJSV compiler or native Oreslang guest. The additional three Node-only regression cases check real SHA-256 vectors,
bounded no-follow file reads, symlink/hardlink rejection, CLI output completeness
and output-parent symlink refusal. Those tests have **not** executed yet on a
GitHub runner and cannot be counted as passed. The build bridge also recomputes
TJSV runId, IR/receipt digest and declaration body digests; self-consistent
forged files still need the upstream pinned `verify-ir` proof against the
actual independent source authorities.

Run all of those
source-locally with pinned toolchains before changing this status.

Further required release gates: actual private Zorde TypeSpec-to-JSON
witness comparison; real Contract IR emitted by TJSV; full codegen
compile and guest execution on Java/Graal + LLVM; property/differential
corpus; memory/ownership proof; WIT component/ABI adapter; hostile
fuzzing; explicit trust-root for the hash-bound receipt. The open source
export incidents forbid private code mirroring to an unapproved public
funded Actions carrier.

## October 19 additional fail-closed gates

Actual admission now requires `configuration.mode=check`, `coverage.sourceMutationCheck=true`, zero structural/differential/emitted findings, and an untruncated finding inventory. Generated plans must bind exactly three well-formed SHA-256 source-authority digests; file ingestion stops when the host cannot supply a nonzero `O_NOFOLLOW` flag rather than silently following a symlink. These logic checks passed in the isolated V8 suite; the real Node crypto/filesystem suite is still pending a source-local runner.

## Reviewed corpus identity and native receipt sealing

The compiler bridge now **requires** a nonempty, independently authored
positive-and-negative JSON instance corpus for *every* admitted scalar. It
reconciles per-declaration counts against the TJSV report, refuses anonymous
or unasserted fixtures, and demands an identical 64-character SHA-256 corpus
identity in `configuration.differential.instanceCorpusDigest`. The
pinned TJSV report deliberately does **not** duplicate the digest within
`report.differential`; that object contains only the summary and the
per-declaration details.

The CLI uses `--instances=contracts/zorde/scalar-instances` to independently
rebuild the pinned upstream `corpusIdentity`: canonical JSON SHA-256 of
ordered `{declaration, expectation, relativePath, instance}` records. Only
regular JSON files in the closed `<Declaration>/valid` and
`<Declaration>/invalid` layout are accepted; symlinks, hardlinks, ambiguous
JSON keys, extra directories and excessive input sizes fail closed. This
check must succeed **before** any generated Oreslang output is published.

The source-local native runner's pre-execution digest manifest includes
**18 inputs**: nine authority/receipt/generated files and nine checked-in
reviewed corpus fixtures. It recomputes the source corpus identity before
native guest execution, and rechecks the full manifest afterward and
immediately before issuing a non-release-qualified native receipt. These
measures depend on a trusted ephemeral build host; a SHA of an
attacker-controlled file does not independently attest who produced it.

## Native guest import-closure sealing (October 19, 2026)

The source-local harness now discovers and seals the **recursive static Oreslang
import closure** of the generated registry smoke and registry specifications,
not only the generated files and independently reviewed JSON fixtures. The
current pilot adds ten implementation/test support modules to the existing
18 source-authority, receipt, guest and fixture inputs (28 total).

Unknown import grammar, missing modules, traversal outside the private source
checkout, symbolic links, excess recursion and duplicate/altered source bytes
must stop execution or the final receipt. The manifest records
`staticGuestImportClosure`; its hash-bound contents are checked after the
guest and before the receipt is issued. The source-free host test checks the
exact expected import graph. This **does not attest the compiler binary's
transitive dependency closure** and cannot replace real native guest tests.
