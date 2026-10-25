# PR #25 — Zorde private-visibility diagnostic binding

Historical PR head `e7509ba0e8e9955588e9eee2f835ae2e292926fe`; current main `62e123bfa0d1f7a26578aa93df8dc56a80efc216`.

The five historical changed paths are **already byte-identical** to those in current main: `docs/ZORDE-COMPILED-REGISTRY.md`, `scripts/check-zorde-denial.py`, `scripts/test-runner.sh`, `scripts/test-zorde-registry-source-local.sh` and `tests/test-zorde-denial.py`. This forward-merge therefore **preserves** current main without overwriting security controls.

Semantic contract: a protected symbol and unrelated private-access diagnostic occurring in the same line must not pass as evidence. Require exact protected-symbol subject/object binding in a single diagnostic and independently bound compiler source identity. A failed or unrelated diagnostic, missing source-local attestation or zero-step CI cannot be accepted as compiler admission evidence.

The repo currently has Python-based legacy audits: do not introduce any new Python scripts. Migration of those checks to Rust is a separate reviewed change, not silently completed by resolving this PR.

**Release:** current security code is already present, but native source-local execution and independent security review are still required. This is a historical conflict cleanup, not a new compilation success.