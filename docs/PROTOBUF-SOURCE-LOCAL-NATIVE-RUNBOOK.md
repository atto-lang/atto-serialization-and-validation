# Protobuf/Zorde source-local native admission runbook

**Status: execution not yet authorized or observed.** The instructions below
describe a fail-closed way for an administrator to obtain *new evidence*.
A zero-step GitHub Actions result is a runner-admission failure, never a
codec, schema parity, guest-test or compiler test result.

## 1. Diagnose runner eligibility first (without exporting source)

The source-free canary
`.github/workflows/zorde-source-org-runner-diagnostic.yml` deliberately has
`permissions: {}`, no checkout, no secrets, no downloads, and one
environment-only step. On the Protobuf PR it is triggered automatically.

Inspect the **actual job steps** and run attempt, not only workflow
conclusion. If `steps=[]`, obtain the administrator's billing/minutes or
spending-limit status, organization and repository Actions policy, runner
availability/eligibility and any workflow restriction. These are separate
possible causes. Do not attribute the failure to the compiler or to
proposed WIT/TypeSpec contracts without executed steps.

The GitHub App connector cannot configure organization billing, protected
environments, source-local ephemeral runners or branch protection. Do not
try private-source export/mirroring to another organization as a workaround:
independent security approval is required, including resolution of the
existing platform confidentiality incidents.

## 2. Prepare a reviewable, immutable source-local native run

An administrator or independent reviewer, **not the candidate run itself**,
must approve each of:

- This PR's exact 40-hex source commit SHA and root Git tree SHA.
- A native Oreslang compiler **executable SHA-256**, and its actual build/
  dependency provenance and authorized installation path.
- A dedicated **source-organization** ephemeral runner, with labels
  `self-hosted, linux, x64, zorde-source-ephemeral` and the protected
  `zorde-source-local-trusted` environment; no unrelated jobs.
- A fresh mode-0700 evidence directory **outside** the private source
  checkout, with an approved private retention policy.

Set these protected environment variables from independently approved
review data, not from the PR's own outputs:

`ZORDE_APPROVED_SOURCE_SHA`,
`ZORDE_APPROVED_SOURCE_TREE`,
`ZORDE_APPROVED_COMPILER_SHA256`,
`ZORDE_APPROVED_COMPILER_PATH`, and
`ZORDE_APPROVED_EVIDENCE_ROOT`.

Dispatch `.github/workflows/runner.yml` **against the approved PR branch**
with `source_local_native=true`. The workflow rejects a mismatched
`GITHUB_SHA` *before* source checkout, then checks out the explicit SHA.
Do not use `main` as an accidental replacement for the reviewed PR head.

## 3. Distinguish first actionable failure

**First executable PR-source boundary:** after checkout, the protected
native workflow itself checks that `GITHUB_SHA`, `git rev-parse HEAD`,
and the root tree equal the independently approved revision, verifies the
physical checkout verifier's bytes against the reviewed Git blob, and runs
the physical source-byte validator in an isolated Python environment.
**Only after this passes** may `test-runner.sh` or any other checked-out
source scripts run. That ordering is essential for a private ephemeral
runner. The workflow source and selected commit still require independent
review before dispatch; the gate does not turn arbitrary PR code into
trusted executables.

`scripts/test-zorde-registry-source-local.sh` independently verifies:
clean Git status, exact HEAD and tree, physical blob bytes, non-symlinked
reviewed compiler path and digest, and protected private evidence directory.

The protected native workflow launches this attester through `env -i` with
an explicit minimal `PATH`, no inherited `BASH_ENV`/`ENV`,
`PYTHONPATH`, `LD_PRELOAD` or `GIT_CONFIG_*` overrides, no Bash
profiles, and only the approved source/head/tree/compiler/evidence inputs.
This reduces configuration-based injection into the attester on a trusted
ephemeral host. It does not establish compiler transitive-dependency
provenance or protect against a compromised self-hosted runner.

It then executes, in order:

1. `tests/protobuf_main.ores`, importing only the codec/Zorde test suites
   and their dependencies. It must exit zero and emit the unique line
   `protobuf-wire-zorde-native-tests-passed`.
2. The full `tests/main.ores` suite through `scripts/test.sh`, which must
   independently emit the same marker, the compiled Zorde registry marker,
   and the full-suite success marker.
3. Positive public-visibility import/execution controls and separate
   negative private-visibility compiler fixtures with targeted diagnostic
   classifiers.
4. The final exact HEAD/tree/physical-source and compiler digest checks,
   followed by an exclusive SHA-bound native receipt.

An early focused failure retains
`protobuf-native-focused.log` inside the protected evidence directory, but
**does not issue a passing native receipt**. An unrelated full-suite failure
retains the focused log and the full-suite log separately; it is not a
successful *full suite*. A successful full run records the independent
focused log SHA-256 in the release-ineligible native receipt.

A candidate commit may change while work continues; any such change requires
**new** reviewer-approved SHA/tree and a **new** source-local execution.
Never apply another commit's passing evidence to the new head.

## 4. Independent contract checks (separate from native execution)

The PR's `.github/workflows/protobuf-contracts.yml` currently targets
hosted Linux runners. When runner admission is restored, require:

- Node static guard with meaningful mutation refusals.
- Checksum-pinned wasm-tools WIT positive parse/binary roundtrip and
  deliberately malformed WIT rejection.
- Independently authored TypeSpec vs. JSON Schema differential corpus,
  positive semantic receipt and negative semantic mutation refusal.
- Artifacts and receipts bound to the source commit SHA. Review the
  results instead of accepting a job label or green check without executed
  steps.

WIT syntax parsing does not implement a component, and TypeSpec/JSON Schema
parity covers only a diagnostic JSON *projection*, not Protobuf wire semantics,
signed/packed/oneof/nested typed codec semantics or compiler-issued
`Valid<T,SchemaId>`.

## 5. Exit criteria

Do not mark the PR ready or merge before actual supported compiler execution,
WIT/TypeSpec evidence, schema-provenance/ownership review and independent
security approval. The source-only lexical guards and source-free canary
are not substitutes. Main-branch protection must be independently verified
by an administrator; GitHub repository metadata previously reported
`protected=false`.

To triage the very first actionable error, report **which of** runner
admission, pinned checkout, approved compiler invocation, import/type
compilation, guest positive assertion, guest negative oracle, schema
differential, or WIT parser failed. Do not collapse them into one CI failure.
