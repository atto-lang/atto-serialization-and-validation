# NetworkCodec<T, F> vs ActorMessageCodec<T>: serialization/borrow contract

**Status (2026-10-15): API declarations + boundary audit.** Both source
interfaces live in `src/schema/transport_interfaces.ores`. Their existence
does **not** automatically register a codec in either compiler or permit a
class to cross any message boundary. Effective admission requires generated
descriptors, a trusted registry, a verified implementation, and runtime tests.

## The two source contracts

`F` is an explicit wire format selector for JSON, MessagePack or Protobuf.
It must ultimately be validated against a compiler-owned closed format set;
current declarations alone do not implement that check. Network formats
must not be sniffed from untrusted input or used to choose actor DATA codecs.
`Sendable<T>` is a compiler-derived ownership/data eligibility proof,
not a user-implementable marker or a consequence of `Serializable<T, F>`.
See [Sendable vs Serializable](SENDABLE_VS_SERIALIZABLE_AND_WIRE_FORMATS.md).

| | NetworkCodec<T, F> | ActorMessageCodec<T> |
| --- | --- | --- |
| Use | TCP/HTTP/WebSocket/RPC/external persistence | supervisor→actor, actor→actor and actor→supervisor **DATA** |
| Input trust | untrusted, arbitrary bytes | trusted compiler schema; sender identity and receiver generation still validated |
| Schema | protocol/content-type/version/format policy, independent from actor identity | closed tagged message domain, exact type/shape digest, version and recipient generation |
| Encode | synchronous read from `borrow T`, output **owned bytes** | synchronous read from `borrow T`, immutable snapshot **before** mailbox admission |
| Decode | validated independent `T` in caller's ownership domain | validated independent `T` owned by receiver's actor heap |
| Failure | recoverable typed error, no partial output | reject before commit; no partial mailbox entry or consumed sender ownership |
| Pointers/capabilities | no addresses, raw pointers, loans, actor tokens, mutexes, channels or closures | same forbidden values, *plus* strict actor sendability/closed type domain |
| Zero-copy | optional pinned borrowed bytes within synchronous scope, no lifetime escape | optional **owned** buffer transfer only with authenticated domain/epoch & completion ACK |

The two interfaces have distinct method names. No subtype relationship, blanket
implementation, auto-derive fallback, or shared implicit `Serializable` marker
may enable both. A type that needs both boundaries must get **two separately
admitted derived implementations**. Format adapters may share stateless
primitive parsers/emitters but not admission permissions.

## The separate reserved CONTROL lane

Supervisor terminal events `on_panic`, `on_raise`, `on_throw` and internal
actor lifecycle events are **runtime CONTROL**, not `ActorMessageCodec<T>`
DATA. Never expose a user-implemented `ControlCodec` to guest code, never
deserialize one from externally supplied bytes, and never let a network codec
name a CONTROL operation. The runtime issues/validates actor/group identities,
epochs, capability guards, event tags and sequence numbers separately.

## Four ownership transitions

1. `rt borrow value` is a **synchronous encoder read lease** only. No store in
   a serializer object or continuation; `await`, `select`, `readch`,
   suspension, callback scheduling, Stream/Observable subscription and
   closure capture while that loan lives must be rejected.
2. `rt take value` is an ownership move. On mailbox send, **do not move at the
   beginning of serialization**: stage/validate from a borrow, reserve mailbox
   capacity and recipient generation, then commit the move and publish bytes
   atomically. Encoding/quota/schema/admission failure preserves the sender's
   value. Cancellation must release any staged owned bytes exactly once.
3. `rt copy value` is a value copy **only for Copy types** or a separately
   verified recursive copy contract. It must never invent another `Pointer<T>`
   owner or become a shallow alias of nested mutable graph values.
4. `rt share value` is only for an explicitly proven, recursively immutable
   `Shared<T>`/frozen backing. It is **not** permission to serialize host
   pointers or skip mailbox snapshot isolation.

`rt ref` gives a unique `Pointer<T>` owner, `rt deref` yields a scoped borrow,
and `rt unref` has a separate proposed release meaning in Java pointer PR
#1022. None of the three is a serialization conversion. Encoding `rt deref
pointer` is permitted only for a **synchronous read of data** while a compiler
loan is active, and only if the codec does not capture the loan. Serializing the
pointer handle, graph address, native slot/generation, FFI capability or borrowed
view is forbidden. After decoder return, payload bytes must not own aliases
into the input. A borrowing/token decoder, if built later, must return a
lifetime-indexed `ValueView<'input>` instead of `T` and must fail closed until
the compiler proves it cannot outlive input, cross suspension or actor turns.

### Serialization and send state machine

```text
Owned sender T ──shared lexical borrow──> validate shape, encode bounded bytes
  ├── on failure: drop staging, release borrow; sender T remains owned
  └── on success: release borrow, reserve queue + actor epoch
       ├── admission failure/cancel: drop staging, sender T remains owned
       └── atomic commit: consume sender T (rt take) / transfer owned payload
            ──release/acquire publication──> receiver
            ──schema/type/version/generation checks──> fresh owned T
```

The exact commit boundary is a compiler/runtime operation, **not** a sequence
of ordinary userland calls that first does `rt take` then attempts enqueue.
This prevents move-before-admission loss and ensures repeated delivery cannot
run guest destructor glue twice.

## Codec integrity requirements

- Network decode must enforce input/output byte quotas, depth, node count,
  UTF-8 validity, noncanonical numbers, map duplicate policy, unsupported
  extensions, numeric overflow, trailing bytes and version negotiations.
- Actor codecs must use compiler-owned immutable schema IDs, a domain-separated
  digest of full types (Array vs List, exact numeric width, enum tag), bounded
  MessagePack v2 or equivalent canonical encoding, and correct actor epoch/
  recipient checks. A shape digest is **not** authentication.
- No dynamic host-class lookup, reflection/deserialization gadgets, foreign
  object methods, arbitrary constructor selection, or open generic erasure.
- ActorRef, ActorGroup, supervisor CONTROL tokens, Future, MutexGuard,
  Channel, Stream, Observable, Fnc, Pointer<T>, borrowed T, and nested forms
  inside arrays/tuples/records/Option/Result/unknown fields are rejected.
  Stable actor *data IDs* may be serialized as integers only after explicit
  schema authorization; those IDs never grant mailbox authority.
- Eager typed decode returns fully owned values. Failed parse or validation
  drops partially constructed values, retains no input pointers or buffer
  leases, and cannot emit partial actor messages.

## Integration blockers / test matrix

1. Compiler parser/type checker recognizes both independently; source
   implementations are validated for signature and recursive payload rules.
   Declaring an interface alone never grants sending privileges.
2. Same positive and negative source fixtures on Java/GraalVM and LLVM:
   primitive/nested round trips; reject borrowed/Pointer fields, forged codec,
   Unknown generic, actor capabilities, cyclic aliases, wrong schema, UTF-8,
   truncated data, budget overrun, stale epoch and mailbox admission failure.
3. Verify same bytes/schema for both implementations *when the actor wire
   protocol intentionally shares a MessagePack representation*. Network
   decoders must not accept an actor DATA envelope as authority; CONTROL
   cannot be encoded by either interface.
4. Test borrow release on throw/raise/panic/defer/cancel and stress concurrent
   send/read/mutate/actor-stop/region retirement under ASan/TSan.
5. Measure copies/allocations separately. `List<int>` accumulation currently
   allocates and copies chunks: no zero-copy/zero-allocation performance claim
   without a pinned owned-buffer backend and exact head CI.

Related: `docs/OWNERSHIP-AND-MAILBOX-SERIALIZATION.md`,
`docs/DYNAMIC-AOT-OWNERSHIP.md`, Java PR #1022, LLVM PR #416,
Java actor MessagePack draft #623 and
`oreslang-source.llvm/docs/audits/MESSAGEPACK_ACTOR_TRANSPORT_AUDIT_20261011.md`.
