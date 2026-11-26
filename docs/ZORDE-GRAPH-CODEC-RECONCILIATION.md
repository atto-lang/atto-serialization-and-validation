# Graph and native codec stack reconciliation

The native CBOR/MessagePack stack (#73) retained an older logical-only
graph envelope. This reconciliation replaces that single host reference
with the exact tested #84 blob 3c346323a0c949475b897939d49e85d83de5251f
from commit 43a928a3a59bc44b7b0351275d5e7c53c4328add. All seven older logical graph tests are retained;
the latest reference adds CBOR framing, owned staging reuse, canonical
rooted ordered topology, bounded cancellation and large-cycle tests.
The existing three guest graph ownership fixtures and all native codec
source blobs on #73 remain untouched. The existing graph workflow must
execute at this new integrated head.

This reconciles source history, not guest semantics. The host reference
has a closed two-type schema and a private CBOR graph profile, not arbitrary
Atto ADT/object graph serialization, object copying, compiler-derived
schema proof, actor graph admission, allocator quotas or native backend
parity. No Python policy exception or production gate change.
