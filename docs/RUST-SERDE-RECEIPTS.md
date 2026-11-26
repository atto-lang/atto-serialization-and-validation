# Rust serialization receipt structure candidate

The candidate preserves the five-backend receipt structure rules and binds
the exact compiled-in manifest bytes, per-fixture input hashes, expected
results, error codes and decoded values. Strict JSON rejects duplicate keys
at every depth and floating-point numeric syntax (all current fixture values
are integers). Run and job identifiers, canonical URLs, native execution
flags, memory bounds, instrumentation and AOT/independent Cranelift claims
are structurally checked. Receipt files are bounded and symlinks refused,
including ancestor directories. Linux trusted system SHA-256 is used; no
new dependency or network access is required. A synthetic receipt can pass:
this is explicitly NOT authentication of execution, logs, artifacts, physical
heap measurements or release readiness. Independent GitHub job and artifact
verification remains mandatory. The existing structural workflow now runs the Rust gate for PRs and main pushes. The old Python CLI and duplicate synthetic suite are removed in this candidate. Invoke `cargo run --locked --bin verify-serde-receipts -- --receipts DIR --head SHA --tree SHA`; flags can appear in any order. This is not a native harness migration. Independent review and exact-head execution of the cutover are required.

The cutover retains all legacy mutation scenarios and adds removed-field, malformed row, hardlink, CLI ordering/duplicate option and post-read inventory controls. Unknown optional fields remain compatible. Trusted isolated receipt directory required; no same-UID adversarial snapshot claim.
