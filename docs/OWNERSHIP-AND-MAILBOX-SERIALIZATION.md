# Ownership-aware codecs and actor-safe serialization

**Domain split (2026-10-15):** network I/O uses `NetworkCodec<T>`;
supervisor/actor **DATA** messages use `ActorMessageCodec<T>`. These
are separate source interfaces with no implicit conversion or automatic
codegen registration. Supervisor `CONTROL` (panic/raise/throw/lifecycle)
is sealed runtime authority, **not** a third user-definable serializer. See
[`NETWORK_VS_ACTOR_SERDE.md`](NETWORK_VS_ACTOR_SERDE.md) for ownership,
rollback, borrowed input, codec signatures and integration gates.

Cross-repo compiler change: [Oreslang PR #411](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/411).

Binding contract: `const x` fixed/read-only; `val x` = `const mut x` fixed/mutable; `let x` rebindable/read-only; `let mut x` rebindable/mutable. Referent mutation still requires exclusive access and recursively checked nested paths.

**Serialization boundaries:** actor mailboxes, RPC, HTTP and durable/network boundaries must encode/decode owned values. Never serialize raw pointers, borrowed references, function/closure identities, live mutex/guard/channel capabilities, or mutable object graph aliasing. Recursive forbidden-capability checks apply to tuples, arrays, optional/union fields and unknown-field escape hatches. Object identity may only be represented through a separately authorized stable ID, never a pointer value.

**Codec APIs:** eager decode returns independently owned memory; borrowed/tape decoders expose immutable views with lifetime bounded by the input buffer. Such views cannot be retained beyond `await`, mutable input replacement, buffer release, or actor migration without a safe promoted ownership transition. Encoding borrowed values may happen synchronously within the active lifetime. Generated code must take/copy/share only via canonical `rt take`, `rt copy`, `rt share`, `rt borrow` checks.

**Struct inference:** `struct infer{foo: "bar"}` is the proposed static structural-literal spelling, distinct from dynamic maps; generated decoder metadata must preserve known field names/types. Do not assume compiler support before PR #411 and follow-up work lands.

**Transport modes:** local execution-domain channels may eventually carry closures/owned object references under proven capture/lifetime rules, but an actor mailbox is always serialized/capability-validated. Do not relax mailbox serializer guards to make local channels permissive.

Conformance matrix: cyclic object graphs, duplicate/unknown fields, hostile nesting, aliasing, nonserializable transitive fields, immutable-view mutation failures, owned decode mutation, cancellation, decoder resource limits, and byte-for-byte canonical fixtures.
