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

## Synchronous generation-fenced host dispatch

`Endpoint::dispatch_next` holds a separate shared mailbox dispatch gate across
binding checks, materialization, the synchronous borrowed handler and value
destruction. All clones share the gate; restart, close and concurrent or
recursive dispatch return Busy without changing generation or consuming a
frame. Mailbox state and tenant locks are not held during the handler, so
trusted enqueue can proceed. Gate poisoning after a handler panic rejects
future fenced dispatch and restart; close can acquire the poisoned gate only
to discard queued frames. Callers must retry Busy according to their own
bounded scheduling policy. The raw receive/pop APIs remain trusted low-level
compatibility interfaces and do not provide dispatch fencing. This is a host
reference only: no asynchronous callback, guest actor, actor heap, scheduler
or external side effect is certified. Actual runtime integration must use
this fence or equivalent across every lifecycle and dispatch entrypoint.

## Owned cooperative suspension fence

`begin_suspended_receive` returns a non-cloneable host delivery with an
owned lifecycle fence and its original logical admission lease. Short
mailbox/gate locking establishes exclusivity; no mutex guard is retained
across suspension. A per-dispatch cancellation handle sets an atomic flag;
old handles cannot cancel subsequent dispatches. Checkpoints refuse further
access after observed cancellation, but cannot revoke existing borrows or
undo side effects. Restart, close and fenced dispatch remain Busy until the
whole delivery is dropped. Field order releases values and credits before
the lifecycle fence. Empty, stale and rejected receives leave no active
fence. This is a cooperative scheduler adapter, not a Future implementation,
forced cancellation, actual actor execution, panic recovery protocol or
physical heap quota. The scheduler must checkpoint on resumption and drop
the task's owned delivery on completion/cancellation before lifecycle changes.
Legacy raw receive/pop remain trusted unfenced interfaces.

## Ordered authenticated-session replay admission

`SequencedEndpoint` is an opt-in strict ordered host profile with one fixed
nonzero 32-byte session identity and a sequence beginning at 1. The trusted
transport must authenticate the session, sequence, endpoint identity, all
binding metadata and payload as one envelope. No cryptographic authentication
is implemented here. Duplicate, zero and out-of-order sequences are refused;
a successful publication alone consumes the sequence. Malformed payloads,
backpressure and failed publication leave it available for retry. Concurrent
admission is serialized by a bounded try-lock; exhaustion at u64::MAX never
wraps. The wrapper does not expose unsequenced admission; the existing raw
Endpoint API remains a trusted legacy interface, not replay protected.
The registrar MUST allocate fresh authenticated sessions on recreation or
restore durable replay state before accepting traffic. This in-memory model
cannot prevent replay after process loss, session reuse or untrusted metadata;
no exactly-once effects, network authentication, guest or heap proof is claimed.

## Materialization-work admission

CBOR ingress now uses allocation-free `preflight_materialization`, which
charges the same logical work as typed decode including UTF-8 copy and array
writes. Validation-only `preflight_typed` remains unchanged. Inputs that
could pass validation but exceed the configured materialization work budget
are refused before publication or sequence commit. The limit applies to
one planned decode; aggregate ingress and receiver CPU work are not yet
metered together. Allocation failures, actual allocator capacity and guest
heap quotas remain independent requirements.

## Cumulative two-pass work ceiling

Endpoint codec `Limits.work` now means the total logical work ceiling for
planning, owned wire copy and receiver decoding (standalone codec limits
retain their original per-operation meaning). To avoid a second planning
traversal or unmetered writes, ingress conservatively reserves one credit
per copied wire byte and caps its allocation-free materialization plan at
floor((total - wire)/2). The plan includes hypothetical writes, so the
same work charge conservatively covers each pass. The admitted frame carries
the exact planned receiver work, and decoding is clamped to that credit and
must report the same usage. Legacy bound frames without a plan are refused
by materialized receive. Failure rolls back reservations and sequence
admission. This intentionally tightens endpoint budget semantics; callers
with old per-pass configurations must migrate their limits. This is a
logical per-message cap, not aggregate tenant CPU throttling, physical heap
accounting, authentication cost, allocator overhead or wall-clock time.

## FlatBuffers scalar-table reconciliation

The #76 restricted scalar-table decoder and schema fixtures are carried into the #86 endpoint stack. Both sequenced and trusted unsequenced entrypoints perform metadata checks, wire reservation, allocation-free preflight, aggregate expansion reservation, exact planned receiver work commitment and owned publication. The cumulative budget is wire copy + twice the planned logical codec work, capped additionally by the FlatBuffers single-pass profile. The decoder returns owned scalar values; no zero-copy borrowing, compiler proof, cryptographic verification, persistent replay, physical allocator quotas or native scheduler integration is implied. The existing fixed-size parser scratch is bounded but its initialization is not separately measured as logical work. The registration digest is trusted rather than compiler-derived. Four endpoint regressions are added; the optional flatc control only provides independent wire evidence when its fixture is supplied. Shape digest reconciliation is handled separately in #93; the benchmark is deferred to a later integration.
