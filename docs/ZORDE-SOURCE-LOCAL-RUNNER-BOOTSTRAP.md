# Zorde source-local native runner: controlled bootstrap

**Status: preparation only; no organization runner allocation or independent
approval is claimed by this file.** The existing source-org hosted Actions jobs
have repeatedly failed before executing steps. Do not infer billing or quota
exhaustion from this symptom without administrator evidence.

## Non-negotiable security boundaries

- Only the existing private source repository can be checked out. Do **not**
  mirror any private Oreslang Git objects, files, executable, branch or test
  artifacts into another organization merely because it has Actions minutes.
  Source-export incidents oreslang-platform-conformance #22 and #77 are open.
- An independent source-org administrator must activate branch protection and
  approve the source-local environment, source HEAD/tree, and compiler build.
  The operator that wrote the PR is not the independent reviewer.
- Provision one **ephemeral**, dedicated, isolated source-org Linux x64 runner
  with the custom label `zorde-source-ephemeral`. Never attach the label to
  a persistent shared host or a cross-org runner. The runner must have only
  the least capabilities needed to execute the approved compiler.
- Configure environment `zorde-source-local-trusted` with required reviewers
  who are independent from the PR author/operator and, where supported,
  branch/ref restrictions. **GitHub may automatically create an environment
  without protection**; merely naming the environment in YAML does NOT create
  approval. Verify the actual protection settings before provisioning the
  runner label. Admin-controlled environment variables are provenance inputs,
  not an independently verifiable authorization by themselves.
- Ensure the source-org Actions policy permits pinned checkout; check billing,
  disabled jobs, repository policy and runner assignment. Checkout uses only
  the repository-local read-only `GITHUB_TOKEN` with persisted credentials
  disabled; no token bridging, public mirrors or downloaded unapproved code.

## Admin-maintained environment variables

Set these values *after* independent review on
`zorde-source-local-trusted`, not in workflow_dispatch user inputs:

| Variable | Required reviewed value |
| --- | --- |
| `ZORDE_APPROVED_SOURCE_SHA` | Exact 40-hex Git commit of PR being tested |
| `ZORDE_APPROVED_SOURCE_TREE` | Exact 40-hex root Git tree of that commit |
| `ZORDE_APPROVED_COMPILER_SHA256` | Exact 64-hex approved compiler **executable** SHA-256 |
| `ZORDE_APPROVED_COMPILER_PATH` | Absolute on-runner executable path; no symlink components |
| `ZORDE_APPROVED_EVIDENCE_ROOT` | Approved private, persistent evidence parent directory outside checkout |

The runner must be trusted, not user-provisioned. The evidence volume must
be readable only by authorized reviewers, and any permissions, cleanup and
retention controls must be independently documented. The compiler executable
digest alone does **not** attest transitive JVM, JDK, LLVM or native-image
dependencies; keep the reviewed compiler build/dependency record separately.

## Trigger after administrative setup

The existing `.github/workflows/runner.yml` supports a manual
`workflow_dispatch` Boolean input `source_local_native=true`.
Dispatch it **against the exact reviewed PR branch** after the protected
environment and ephemeral runner are configured. The job:

1. Fails before checkout when the environment-approved source SHA/tree or
   compiler digest/path is empty, malformed, or the approved source SHA does
   not equal the GitHub event commit SHA.
2. Checks out that SHA only within the *same* private source repository,
   with read-only repository token permissions and no persisted credentials.
3. Runs shell syntax, export closure and source-free refusal contracts.
4. Creates a private mode-0700 evidence directory outside checkout.
5. Runs the pinned actual compiler via
   `scripts/test-zorde-registry-source-local.sh` with exact source HEAD,
   tree, executable hash, physical blob checks, public import/execute
   controls, and two genuine private-member visibility diagnostics.
6. Emits a non-release receipt and its hash into the source-org job summary.
   Detailed guest logs stay in the approved evidence parent; they are **not**
   pasted into the Actions console or copied to an alternate organization.

The job must have actual executed steps and a traceable evidence directory;
success without source-compiler logs and independent review is not release
qualification. Its receipt explicitly marks transitive compiler dependency
closure, cross-actor ownership and backend parity **uncertified**.

## Blocked/failed semantics

- Zero steps / no logs / no runner: `NOT_EXECUTED`, never a test failure.
- Missing environment review, approved variables, or ephemeral runner: refuse
  provisioning/testing; do not relax checks to obtain a green workflow.
- Nonzero source-free refusal suite: **harness broken**, do not run compiler.
- Nonzero compiler, missing native success markers, or failed positive import:
  **native not qualified**; preserve private logs.
- Privacy fixture rejected by unrelated syntax/path error: **no encapsulation
  evidence**. The same compiler must resolve and run both public controls.
- Unreviewed source export, mutable compiler, or altered Git object/bytes:
  **security incident / evidence refusal**; do not promote a receipt.

A successful dispatch qualifies only the specified commit/tree/compiler
combination. New pushes require a new protected approval and fresh run.
