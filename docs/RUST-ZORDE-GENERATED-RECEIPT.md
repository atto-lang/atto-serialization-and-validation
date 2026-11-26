# Rust generated guest process receipt candidate

Combines bounded process capture, explicit authority digest binding, exact
tracked source bytes, independently pinned compiler and Node executable
hashes, original manifest and post-execution closure recheck, static
consumer/IR/plan/build marker consistency, unique exact success marker and
exclusive private receipt output. A caller cannot pass a success status or
prewritten log to the API. Refuses failed/missing marker, changed source,
mismatched approvals and existing output. All synthetic controls are host
only. The candidate deliberately uses a distinct schema and retains
native_guest_executed=false, release_ready=false; running an approved binary
does not independently prove guest semantics. The production gate must
still establish trusted runner and dependency provenance, generated artifact
inventory, actual compiler semantics and independent review before using
the production schema. The old Python gate is unchanged. The tracked-only
verification mode does not attest untracked/generated files outside the
manifest. Trusted exclusive checkout, parent and process isolation remain
mandatory; no atomic hostile same-UID snapshot is claimed.
