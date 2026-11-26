# Static contract regression recovery

The 2026-11-25 exact-head integrated job 45689374785 recorded 11 failures in
92 Node tests. The CLI positive fixture described only Port despite the
reviewed pilot requiring Port and ShortLabel; native guard tests omitted
mandatory Node approval inputs and expected an outdated manifest count of 18
rather than 19; the nested Node TAP test inherited NODE_TEST_CONTEXT, which
causes Node's recursive test-runner protection to skip test files and emit no
TAP summary. This is reproduced independently with Node 22.16.0.
The fix retains the single-declaration generic tests, supplies a distinct
nine-instance two-declaration pilot fixture, checks both missing and malformed
Node digests, and removes the inherited internal test context before invoking
the real test runner. No TAP acceptance criteria, source approval conditions,
or pilot descriptor checks are loosened. Synthetic tests remain host-only.
The integrated workflow still must execute against the exact new head with
pinned Node 22.18.0; only that run can establish static pipeline recovery.

The first exact-head retry executed 94 cases with one failure (job 139601025411). The remaining synthetic checkout case failed in git commit after a previous symlink case. Reset test/module leaves between cases so writes cannot follow a previously installed symlink, and preserve Git stdout/stderr on fixture setup failure. The denial cases and production checks remain unchanged.
