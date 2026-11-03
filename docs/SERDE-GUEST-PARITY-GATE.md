# Fail-closed JSON/MessagePack guest verification gate (PR #51)

This document defines **requirements**, not executed guest evidence. AttoLang
new-language-source fixtures should use `.atto`; the changed `.ores` test
suites are retained as legacy integration entry points during migration.

## Frozen fixture boundary

- `tests/main.ores` invokes both `run_json_tests` and `run_messagepack_tests`.
- `fixtures/serde/adversarial-v1.json` records 21 byte-exact shared vectors:
  19 negative and two valid-neighbor controls.
- `python3 -B scripts/check-serde-fixtures.py` fails on any manifest/guest
  byte drift, missing guest invocation, missing `is_err()` assertion, or
  mismatch in any of the **19** source-authored typed error diagnostics.
  `tests/source_free/test_serde_fixture_gate.py` tests its fail-closed
  behavior against deliberate tampering.
  **These checks are source/host-only; never report them as guest PASS.**
- `DecodeError.code` is the cross-backend error taxonomy. The JSON guest
  suite asserts all seven negative codes and the streaming exponent EOF
  transition; the MessagePack suite calls the typed `decode_attempt()` API
  on all 12 malformed vectors. Actual execution and agreement between the
  five backend compilers remain required.

## Actual native guest gate

Execute the same input bytes and expected error classifications with:

1. Java/GraalVM real Oreslang/AttoLang guest compiler and guest runtime,
   and full Native Image when claiming AOT.
2. LLVM native guest compilation and execution, with ASan/UBSan where available.
3. Wasm guest compilation and actual module execution with enforced memory
   bounds; invoking a host-side JSON/MessagePack parser is NOT sufficient.
4. Independent Cranelift compiler and actual guest execution (neither
   Wasmtime alone nor LLVM masquerading as Cranelift).
5. .NET **NativeAOT** compiled guest executable (managed-only execution is
   insufficient for NativeAOT claims).

Require the existing positive integration suite plus the 21 manifest cases.
For every negative vector, the guest must return `Err` with the expected
`DecodeError.code`, with no crash, hang, oversized allocation or artifact
promotion. For positive vectors, the guest must return `Ok` and preserve
the expected value/collection structure. Limit depth and bytes/elements,
and supply measured allocation/peak-memory evidence and native sanitizer
or equivalent instrumentation. Unsupported feature lowering is
`UNSUPPORTED`, not `PASS`, and cannot publish an approved guest artifact.

## Evidence capture and publication

For each backend capture immutable source HEAD and Git-tree SHAs; fixture
manifest SHA-256; exact input SHA-256 for every case; compiler identity,
version, binary SHA-256 and target; guest artifact SHA-256; job/run IDs,
nonzero executed step count, actual command and stdout/stderr/exit;
case error taxonomy; bounded depth/allocation; and instrumentation receipt.

`python3 scripts/verify-serde-receipts.py --receipts PATH --head FULL_SHA
--tree FULL_TREE_SHA` performs a **structural** five-backend manifest gate.
It rejects missing lanes, empty step counts, omitted case results,
SHA mismatches, host-only jobs, missing instrumentation and unverified
resource bounds. It does **not** independently authenticate a reported
GitHub job or binary: a reviewer must verify the API-reported run SHA,
steps and artifact digest, toolchain identity and logs. Do not publish
release-grade artifacts based solely on self-authored JSON receipts.

Only allow the final artifact-publishing step after both structural
validation and independently verified execution/provenance gates pass;
retain failures as private diagnostic records and never overwrite older
verified artifacts. If jobs fail with `steps=[]` or `steps=null`, report
`INFRA_FAILED` or `NOT_EXECUTED`, **not** decoder failure. Store no source,
private error payloads, credentials or tokens in public logs/artifacts.

## Alternate private runner policy

The source repo is private. A test organization's `GITHUB_TOKEN` cannot
checkout this repo across organizations without delegated access.

**Hold:** oreslang-platform-conformance issue #320 requires independent
destination approval and resolution of source-protection/incident holds
before *any* private source export. Do not perform a cross-org mirror
until that decision has been independently documented.

After authorization, use only a private, access-controlled ephemeral test
repo in an explicitly approved alternate organization. Check for recent
*stepful* successful Actions admission there. Mirror the exact approved
source tree using Git blobs/trees into an isolated short-lived branch;
verify every transferred blob SHA, root tree SHA, and immutable source HEAD
mapping before build. Do not clone the private source from Actions using
secrets or a foreign repo token. Avoid privileged triggers, unpinned
third-party Actions, persist-credentials, untrusted mutable dependencies,
or publishing private guest artifacts. Use `contents: read` and the test
repo's own read-only checkout, retain private attestations and delete the
temporary mirror under the approved retention policy.


## Source-local executable admission (manual, subject to admin approval)

The existing default-branch workflow `.github/workflows/runner.yml` now accepts
the `serde_source_local` manual input, mutually exclusive with the separate Zorde
`source_local_native` input. It never runs private-source guest execution on PR
or push events. An administrator must provision the protected
`serde-source-local-trusted` environment and a short-lived SOURCE-ORG runner
with the exact `serde-source-ephemeral` label. No organization-to-organization
private-source transfer is permitted by this path.

Admin-controlled environment variables, NOT untrusted workflow inputs:

- `SERDE_APPROVED_SOURCE_SHA`: immutable PR source commit SHA
- `SERDE_APPROVED_SOURCE_TREE`: immutable root Git tree SHA
- `SERDE_APPROVED_COMPILER_SHA256`: independently approved compiler digest
- `SERDE_APPROVED_COMPILER_PATH`: installed trusted absolute compiler path
- `SERDE_APPROVED_EVIDENCE_ROOT`: private trusted absolute path outside checkout

Before enabling manual dispatch, independently validate those pins and
the runner/environment's source-protection permissions. Select the PR
branch and `serde_source_local=true`, with `source_local_native=false`.
The job checks approved SHA before checkout and then runs
`scripts/test-serde-source-local.sh`, which performs raw Git HEAD/tree/index/blob
validation, refuses untracked/ignored/symlinked paths and wrong executable
modes, exercises the source/host tamper suite, verifies the approved
compiler SHA-256, and runs the actual `tests/main.ores` guest integration entry.
Only a zero-exit guest run with the exact completion marker can emit a
private, non-promotable source-local receipt. The source tree is rechecked
after running; logs remain within the approved private directory.

The source-local receipt explicitly records `backend=UNVERIFIED`,
`full_aot=false`, `asan_ubsan_verified=false`,
`five_backend_parity=false`, and
`artifact_promotion_allowed=false`. An approved compiler digest and
a guest stdout marker do not on their own prove backend provenance or
NativeAOT. A reviewer still needs job steps/logs and independent compiler
identity verification.

Local host-only verification of the raw checker and admission refusals
does not close any native guest gate. For five-backend PASS, separately
collect the full per-case immutable receipts and backend-appropriate
instrumentation described above.

No cross-org transfer, native guest execution, sanitizer PASS, binary
compatibility or five-backend parity is claimed by these source changes.
Independent review remains required before merging PR #51.
