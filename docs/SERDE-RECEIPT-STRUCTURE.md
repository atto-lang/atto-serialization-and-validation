# Serde adversarial receipt structure (host-only)

Independent subset of [draft #26](https://github.com/atto-lang/atto-serialization-and-validation/pull/26). The 21 JSON and MessagePack vectors are authored expectations, not executed guest results. The five original backend receipt lanes are a historical baseline, not a claim of all current backend parity. This tool checks only receipt structure; independent run/job/artifact verification and actual native guest execution remain mandatory. It does not change codec source, runner policy, or the canonical guest suites.

Run the synthetic host-only regression suite with `python3 -B tests/source_free/test_serde_receipt_gate.py`. A structural pass alone must never certify a release. Native typed diagnostic fixtures, guest execution and expanded backend coverage remain separate.
