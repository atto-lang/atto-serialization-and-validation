# Pinned actor DATA ingress host reference

Compose fixed registration-time codec, nominal schema, version, shape digest and receiver generation with allocation-free typed preflight and a generation-fenced mailbox reservation. Only a trusted transport may call the post-authentication entrypoint using exact authenticated metadata and immutable payload. This module does NOT authenticate peers, verify a MAC, derive the descriptor digest, enforce replay protection or bind endpoint identity cryptographically. Those remain external prerequisites; synthetic metadata in tests is not authentication evidence. The caller must bound ingress snapshot storage and account receiver materialization separately; decoded values are exposed only by the host materialized-delivery API described below. CBOR preflight and Protobuf preflight use their existing independent logical limits; no fresh decoding budget is implicitly authorized. Mailbox tenant credit covers owned wire bytes and follows delivery. No CONTROL authority or compiler guest claim.

## Shared logical expansion admission

The tenant also carries a separate logical expansion ceiling, configurable via
`TenantBudget::new_with_expansion` (existing `new` defaults to 256 MiB).
After allocation-free typed preflight, ingress reserves the reported CBOR
expanded or Protobuf logical cost before copying or publishing bytes. This
credit follows the wire delivery until it is dropped, including across
endpoint restart. Failure releases all unpublished wire and expansion
credits; queued closure releases queued credit only. Generic wire-only
mailbox users retain their original API. This is a conservative cross-codec
logical admission meter, NOT physical heap accounting or permission to
materialize without receiver-side allocation quotas. The consumer must not
retain decoded values after releasing the admission lease unless separately
accounted. Authentication, guest execution and runtime integration remain
external prerequisites.

## Bound receiver-side host materialization

`Endpoint::receive_materialized` dequeues only an admission from the same
registered endpoint instance using a private in-process Arc identity. A
mismatched or generic unbound queue head remains queued; trusted integration
should dedicate a mailbox to each endpoint. The legacy raw `Mailbox::pop`
remains a trusted compatibility interface, not an authority boundary.
The decoder's logical ceiling is clamped to the reserved expansion and its
reported usage must match exactly. The owned, non-cloneable result retains
the original wire delivery and all its tenant credits, and exposes only a
borrowed host value. Consumers must account their own copies separately.
On failure the unpublished value and reservation are dropped. Final generation
check is an observation before return, not atomic actor dispatch; actor runtime
must fence use under its own generation lock. No physical allocator quota,
nominal guest conversion, transport authentication or runtime integration
is certified by this host reference.
