# Zorde compiled scalar registry — phase 1

**Status:** experimental native Oreslang guest implementation on a stacked PR,
not a release-ready TypeSpec/JSON Schema admission engine.

The primitive compiled registry is implemented in
[`src/zorde/compiled_registry.ores`](../src/zorde/compiled_registry.ores),
stacked on [Zorde facade PR #14](https://github.com/ores-truffle-oreslang/oreslang-serialization-and-validation/pull/14).
It **reuses** `zorde_parse_scalar_metric` and the compiler-aligned
`ZordeScalarRule`/canonical scalar interval kernel. It does not interpret an
arbitrary JSON Schema, execute dynamic `where` predicates, generate a user
nominal type or run a second constraint semantics engine.

## Explicit authority/trust boundaries

1. Authored TypeSpec and independently authored JSON Schema are the **peer
   authorities**. A review-pinned TJSV build/CI comparison and differential
   fixtures must succeed before an external loader may claim schema admission.
2. A compiler/loader must separately authenticate schema digests, normalized
   descriptor version, nominal identity and source revision. **This phase-1
   public function is not an authenticity check**: any caller can supply
   descriptors and obtain a registry. Never infer trusted schema provenance
   from successful compilation alone.
3. `zorde_compile_scalar_registry` checks a restricted descriptor subset and
   builds a private in-process snapshot. It refuses invalid metadata and
   returns a typed failure report without retaining secret payload values.
4. `zorde_registry_parse_scalar` selects an already compiled scalar and
   returns `Result<int, ValidationReport>`. It does **not** compile TypeSpec,
   parse JSON Schema or recompute authority equivalence per payload.
5. A registered scalar returns only its exact numeric metric or Unicode-scalar
   string length. It does not yield a forgeable `Valid<T>` proof or imply that
   the original dynamic `Value` cannot be mutated after validation.

### Fail-closed restrictions

| Property | Phase-1 bound |
| --- | --- |
| Registry entries | 1–64 |
| Schema identifier | 1–128 printable ASCII bytes (no NUL/control bytes) |
| Scalar exclusions | 0–256 per entry |
| Signed-int64 numeric lexeme | at most 20 bytes, checked before parsing |
| UTF-8 scalar input | at most 65,536 bytes, checked before scanning |
| Supported descriptor | canonical v1 exact signed int64 or UTF-8 Unicode-scalar string length |
| Duplicate selectors | refused |
| Unsupported descriptor version/kind/property, invalid intervals, unsorted/duplicate/off-domain exclusions | refused |
| Unknown schema selector | refused |
| Floating-point coercion of signed int64 | never allowed |
| Remote/dynamic references, schema recursion, JSON Schema general vocabulary | **not implemented / not admitted** |

Source metadata and ID byte lists are deep-snapshotted at compilation, and the
private entry class is not exported. The current bounded linear lookup is
O(64) worst-case; it is a correctness-first AOT-friendly kernel rather than
a claim of a high-performance hash-indexed registry.

## Ownership and actors

This library constructs an ordinary class value, **not an actor**. Actor
persistent data still belongs in its single `state { ... }` schema and is
accessed only through verified state loans/critical APIs. A registry may be
owned by one execution domain; cross-actor sharing requires the compiler to
prove recursively immutable memory with `rt share`, not merely `val`
references around mutable `List` values. The current PR does not certify
concurrent actor sharing, atomic generation switches or rollback.

## Native test plan and evidence limitations

`tests/zorde_compiled_registry_test.ores` is invoked from `tests/main.ores`.
The suite verifies accepted/refused scalar inputs, exact 2^53+1 and
20-byte INT64_MIN, UTF-8 scalar length and invalid encodings, oversized
numeric/UTF-8 payload refusal, maximum-length selectors, case-sensitive
schema lookup, duplicate/unknown IDs, defensive input snapshots,
descriptor version errors, capacity and complexity ceilings.

With an approved local Oreslang compiler:

```sh
ORESLANG_BIN=/path/to/oreslang ./scripts/test.sh
```

At the time of writing, **source-org GitHub Actions jobs fail before any runner
step** even for a source-free/no-checkout diagnostic. This source has not yet
executed under Graal, LLVM or Wasm and should not be counted as native
conformance. Do not export it to a separate GitHub org to obtain minutes
without independent protected-main exact-source authorization; see
[private CI bootstrap #69](https://github.com/ores-truffle-oreslang/oreslang-platform-conformance/issues/69)
and the open security incidents #22 and #77.

## Next phase

- Build a closed **generated static Contract IR** with SHA-256-bound peer
  authority digests, compile-time recognized vocabularies and unsupported-
  keyword refusal, using Zorde parity PR #16.
- Add a trusted-only compiler/loader admission capability with generation
  IDs and frozen registry data; do **not** expose an untrusted bool
  `admitted=true` or digest string as authority.
- Compile bounded object/array/string rules using the existing
  `src/validation/validation.ores` kernel, enforcing depth, bytes, errors,
  fuel and cancellation; preserve exact integer lowering on its scalar path.
- Prove actor snapshot ownership, atomic generation swap, rollback and
  cross-backend Java/LLVM/Wasm parity; exercise adversarial property/fuzz
  corpus and benchmark allocation/time budgets.

## Independent source-free reference tests (not native execution)

The independently authored, synthetic-only scalar mathematical oracle runs on
an available funded runner in [sagitta-infra PR #38](https://github.com/sagitta-stack/sagitta-infra/pull/38).
Its deterministic seed drives 20,000 interval/property probes, exact signed
int64 boundaries, malformed UTF-8, and two deliberate faulty references
(exclusive upper bound and ignored exclusions). Its receipts explicitly
state `actual_oreslang_compiler_executed=false`,
`actual_oreslang_runtime_executed=false`, and
`private_source_transferred=false`.

That test validates a future **independent expectation oracle**, not the
behavior of this private Oreslang implementation. No private Git tree, Oreslang
source file, binary or WIT file is copied into the funded organization.
The source-repo exact-head native tests remain blocked until GitHub Actions
schedules runners within the authorized source trust boundary. Even an exact
private Git-tree SHA match in another organization cannot substitute for
the required independent source-export authorization and incident clearance.

## Approved source-local native evidence runner

The source-org remains unable to allocate hosted jobs (even a zero-checkout
canary), so do **not** mirror this private Git tree into an alternate org without
the separate independently reviewed, exact-revision source-export approval.

For a trusted, ephemeral runner **inside the source organization's trust
boundary**, use [`scripts/test-zorde-registry-source-local.sh`](../scripts/test-zorde-registry-source-local.sh).
Its required immutable identifiers are the reviewed repository commit SHA,
its exact root Git tree SHA, and the SHA-256 of a separately approved,
executable Oreslang compiler. The script
refuses dirty checkouts, wrong HEADs, unexpected executable digests, missing
executables, missing required environment variables, absent native test
markers, nonzero process exits, and evidence output inside the checkout. It
caps the positive execution at 300 seconds, then requires two compiler-negative
privacy fixtures under `fixtures/zorde-compile-negative/` to fail with specific
access-control diagnostics rather than an arbitrary parser error. It creates
a SHA-bound, non-release machine-readable receipt **only after those checks
and the full native suite have actually succeeded**.

```bash
mkdir -p /var/tmp/zorde-native-evidence
export EXPECTED_SOURCE_SHA='<independently approved 40-hex PR head>'
export EXPECTED_SOURCE_TREE='<reviewed exact 40-hex root Git tree>'
export EXPECTED_COMPILER_SHA256='<independently approved 64-hex executable digest>'
export ORESLANG_BIN='/absolute/path/to/approved/oreslang'
export ZORDE_EVIDENCE_DIR='/var/tmp/zorde-native-evidence'
bash ./scripts/test-zorde-registry-source-local.sh
```

Do not fill either expected digest by blindly reading the current unreviewed
workspace or by trusting a self-reported test harness. Compiler executable
SHA-256 does **not** attest dependency closure for an executable launcher,
Graal/JVM artifacts, LLVM backend, SDK, or native-image runtime; the receipt
marks that gap explicitly. Source-level and transitive compiler provenance,
authorized environment identity, separate output review and full backend
conformance are still required for release.
