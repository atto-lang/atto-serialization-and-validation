# Sendable vs Serializable vs actor DATA transfer

Status: design + source declaration / static guard only (2026-10-15).
Nothing in this document is a claim that generic source codecs execute yet
in Java/GraalVM or LLVM. Native conformance, runtime codec registration,
compiler derivation and atomic actor-send lowering are outstanding.

## Distinct guarantees

| Concept | Proof | Does not imply |
| --- | --- | --- |
| `Sendable<T>` | Compiler verifies transitive owned-data eligibility and an actor-boundary ownership transition. This is **not** a user-implementable trait, annotation, reflection marker or network codec. | Encoding in a particular wire format, having mailbox authority, or enabling shared mutation. |
| `Serializable<T, F>` | **Conceptual** format-indexed property: a verified codec for exact `T` and one explicitly selected format `F`. No public source trait or blanket derive is enabled. | `Sendable<T>`, actor mailbox admission, runtime CONTROL. |
| `NetworkCodec<T, F>` | User-visible interface proposal for untrusted network bytes. `F` is a format marker chosen by the protocol/caller, never sniffed from attacker bytes. | Permission to send `T` to an actor, deserialize actor envelopes, or delegate actor IDs. |
| `ActorMessageCodec<T>` | Actor DATA encoder/decoder with compiler/runtime-owned schema descriptor. No guest-controlled schema-ID method. | CONTROL privileges or network decoder privileges. |
| Supervisor CONTROL | Runtime-issued authenticated lifecycle/terminal events on reserved channel. | Any guest-implementable or network-deserializable codec. |

**Important:** The `NetworkCodec<T, F>` source declaration is a type-level
contract, not proof that `F` already has an enforced closed-format registry.
The compiler must eventually restrict `F` to explicit built-in descriptors
for JSON, MessagePack and Protobuf, with no `Dynamic` fallback. `@Sendable`
and `@Serializable` must fail closed until proof-checked derive is implemented.

## Format policy (future runtime requirement)

| External format | Required validation |
| --- | --- |
| JSON | UTF-8 and Unicode policy, duplicate object key policy, strict numeric range/precision, bounded depth/bytes/nodes, disallowed nonstandard NaN/Infinity, no trailing garbage. |
| MessagePack | Integer width and extension-type allowlist, canonical/minimal width if canonical mode, validated text UTF-8, map key policy, bounded binary/container lengths, no trailing bytes. |
| Protobuf | Exact generated schema + field numbers/wire types, varint overflow and overlong handling, recursion/field/byte budgets, deterministic duplicate/oneof policy, unknown fields retained only as **inert data**, no reserved CONTROL interpretation. |

`NetworkCodec<T, JsonWire>` and `NetworkCodec<T, ProtobufWire>`
(illustrative marker names, **not implemented built-ins**) are distinct
specializations. An endpoint must explicitly bind its accepted content type,
wire version, and exact codec; no heuristic format detection may infer an
actor message or choose a privileged decoder. Format-specific code may
share pure scalar UTF-8/integer primitives; it must not share authorization.

## Network ingress to actor DATA

1. Treat all incoming network bytes, headers, schema IDs, actor names and
   message tags as untrusted. Apply transport permissions, framing,
   byte/depth/allocation budgets, version and schema validation.
2. Decode via the **explicitly selected** `NetworkCodec<T, F>`, into a
   fully owned value in the current domain. A borrowed byte view must not
   be returned as an owned `T`.
3. Validate/map network DTO into an explicitly allowed *closed* actor
   message domain. Unknown `Value` shapes, host capabilities, pointers,
   arbitrary object graphs and payload-directed actor destinations fail
   unless the program explicitly validates/reifies them.
4. Compiler verifies `Sendable<T>` and an ownership operation:
   `rt borrow` only for synchronous staging; `rt take` only at atomic
   successful commit; `rt copy` only for verified deep-copyable values;
   `rt share` only for frozen recursively immutable values.
5. Compiler/runtime actor descriptor supplies DATA shape, schema digest,
   recipient identity/generation/epoch, capability and admission policy.
   The sender cannot provide a schema digest as an authority token.
6. Snapshot/encode, reserve mailbox budget, verify epoch, then commit and
   publish atomically. Rejection/cancellation rolls back without consuming
   sender ownership. Decoding yields fresh receiver-owned data.
7. The supervisor may issue ordinary DATA messages by the same DATA rules,
   but `on_panic`/`on_raise`/`on_throw` and lifecycle CONTROL events
   can only originate from the sealed runtime CONTROL lane.

Never route a decoded network envelope directly into the supervisor CONTROL
channel, and never treat an arbitrary actor-data destination string, stable
ActorId integer, nonce, schema hash or Protobuf unknown field as authority.

## Fan-out, round robin, and reply streams

- One-to-one and round-robin send each have one recipient/atomic admission.
- Fan-out must either create **separate owned snapshots for each recipient**
  or transfer a proven immutable reference-counted buffer with independent
  receiver leases, epoch fences and completion ACKs. One mutable graph must
  never become aliased writable state in multiple actors.
- Failed admission at one destination must have an explicit partial-success
  policy and never silently restore consumed ownership from prior commits.
- Replies from supervisor/actor pipelines remain DATA and must carry runtime
  provenance and correlation independently from untrusted payload bytes.

## Required tests (not yet claimed)

- Compile-time: reject manual/fake Sendable, inert derive annotations,
  NetworkCodec objects as messages (including module-qualified and nested),
  pointer/loan/closure/ActorRef/Future/Channel/Stream/Observable/unknown types,
  cross-format or cross-domain decoder substitution.
- Hostile inputs for all formats: truncated frames, overlong varints,
  invalid UTF-8, duplicate keys, numeric overflow, wrong type/schema,
  unknown tags, quota overruns, forged CONTROL and trailing bytes.
- Runtime: send failure preserves sender ownership; commit exactly once;
  cancellation, stale recipient epoch, stop/restart ABA, moved buffer lease,
  nested graph mutation, fan-out alias safety and backpressure.
- Java + LLVM: same data schema digest and actor MessagePack v2 canonical
  vectors; compiled supervisor→actor→supervisor and actor→actor DATA
  round trips; no claim of zero copy without native copy/allocation proof.
- Exact-tree test runs are required before merging, including JDK 25/GraalVM
  and native compiler/ASan/TSan where supported.

See `NETWORK_VS_ACTOR_SERDE.md` and
`src/schema/transport_interfaces.ores` for the narrower source contract.


## Erased callable values are not portable data

`AnyCallable` is a sealed compiler-intrinsic erased callable **type**, not
a user-implementable trait or interface, `Sendable` proof, codec, or schema.
Reifiable concrete functions may widen to it for in-process callback
passing, but the erased type loses its known call signature: no direct
invocation and no dynamic properties. A class with a `call` method does
not become an `AnyCallable`.

No `AnyCallable`, `Fnc<...>`, closure, bound callback, partial-constructor
`self` capture or executable function table may be encoded as
`NetworkCodec<T,F>` or `ActorMessageCodec<T>` data simply because its
type is structurally callable. Callable environments may retain handles,
borrows, mutable state, actor references, and supervisor privilege; codec
registration cannot turn these into Sendable/Serializable owned payloads.
Recursive containers and module-qualified aliases do not weaken that rule.
The reserved supervisor CONTROL channel is never a deserialization target.
If executable remote behavior is needed, use a separately authenticated
protocol command/schema and perform permission checks on the receiver,
not byte-level function or closure transport.

All backends lacking the compiler/runtime enforcement for `AnyCallable`
must reject the feature explicitly until real ABI and ownership conformance
is demonstrated. Contract audit only verifies source-level reservations,
not actual portable codec generation or callable type checking.
