# Native dynamic values, generated codecs, ownership and AOT gates

Status: **implementation contract + partial native hardening, not a completed AOT or runtime-conformance certification** (2026-10-09).

## 1. Different requirements, different decode paths

| Input / target | Native representation | Compile-time dependency |
| --- | --- | --- |
| Unknown JSON | `Value` tree (string-key objects, nested arrays/scalars) | JSON parser and closed `Value` ABI |
| Unknown MessagePack / CBOR | `Value` tree (string-key objects **or** non-string-key `Value` maps) | Format parser and closed `Value` ABI |
| Unknown Protobuf | Numbered `ProtoField` wire fields, with raw unknown-field bytes | Wire decoder only; **not** an invented object/map schema |
| Known Oreslang class / struct | Statically generated specialized constructor/decoder | Build-time schema, class/struct layout and generated codec |
| Known Protobuf message | Statically generated field-number dispatch, including nested/packed/map/oneof | Compiled descriptor / `.proto` / TypeSpec metadata |
| Unknown top-level scalar or array | `Value` of corresponding kind | None |

The current `Value` object stores string-key map entries as an ordered
`List<WireMember>` to preserve duplicate-key policy and wire ordering. This is
**map semantics**, not yet a native `Map<String, Value>` materializer. Do not
claim a `Map<String, Value>` API exists until a deterministic native
UTF-8-to-String conversion and key-equivalence contract is implemented and
tested; using `Map<Utf8Text, Value>` without semantic equality/hashing could
silently make equal wire keys distinct by reference identity.

The `value_lookup` and `value_lookup_path` functions provide **native,
reflection-free navigation** without first converting a wire object to an
Oreslang `Map`. Path segments are `Value` strings for object keys, integer
numbers for arrays, and arbitrary `Value` keys for general binary maps.
The path is bounded (128 segments). These accessors are linear in object/map
size; generated typed decoders should use field dispatch instead.

## 2. Tree shape and AOT behavior

Incoming wire trees can nest without any fixed object shape, e.g.:

```text
object
  -> array
     -> object
        -> array
           -> object
              -> bool
```

The payload cannot encode shared object references or cycles in JSON, ordinary
MessagePack, or the currently implemented definite-length CBOR subset.
**Programmatically constructed Oreslang `Value` instances are a separate
case**: their public collection fields are mutable, and can acquire aliases or
cycles unless the compiler/runtime enforces ownership. Depth-limited encoders
are not a substitute for a sound ownership/cycle policy.

AOT compatibility means: every codec body, primitive operation, metadata table,
constructor and dispatch path is emitted/linked ahead of time; dynamic payload
*contents* are still unknown until runtime. It does **not** require discovering
new Oreslang classes or bytecode at runtime.

Typed decode must be emitted as ordinary Oreslang source or compiler IR:
1. Validate static metadata at build time; require unique source/wire names and
   legal Protobuf field numbers.
2. Dispatch directly on constant field names or numbers.
3. Decode child values with concrete generated `Codec<Child>` calls;
   recursively specialize arrays, maps, options/unions, structs and classes.
4. Check missing vs explicit null vs default separately, and enforce numeric
   narrowing/overflow, required/unknown/duplicate policies.
5. Allocate and construct the concrete Oreslang class/struct only after all
   required and nested validation passes.
6. Encode by direct field access; preserve Proto unknown fields only when
   requested by the type contract.
7. Prohibit runtime reflection, JVM/Jackson/Gson/protobuf-Java materialization,
   runtime class lookup, dynamic compilation and classpath scanning.

An open generic `decode<T>` requires the compiler to resolve `T` at
build-time or a closed generated registry of concrete `T` codecs. A
schema-less Protobuf message can only become a raw wire-field collection; a
descriptor is necessary to give bytes semantic field names and nested types.

**Current gap:** the repository has static metadata and a representative
generated `User` codec test, but it has not yet proven production
compiler-driven generation, direct-to-struct decode, nested Protobuf maps/
oneofs, or GraalVM native-image/WASM/native AOT end-to-end.

## 3. Actor, borrow and ownership rules

A decoded tree belongs to a receiving logical actor or supervisor execution
lane. Parsing must produce owned values, not host/JVM object handles.

- `rt borrow`: non-owning read while the underlying owned bytes/tree stay
  alive; the compiler must reject mutable aliasing and unsafe borrows across
  suspension points such as `await`.
- `rt take`: transfer an owned `Value` tree into the receiving actor's
  mailbox, invalidating the sender's access. A nested owner must transfer too.
- `rt share`: allowed across parallel actor lanes **only after** the
  complete transitive tree becomes immutable (including all nested lists,
  objects, byte buffers, and map entries).
- `rt copy`: explicit, bounded **deep** copy when mutable independence is
  required; an aliasing/shallow copy is not sufficient for nested values.

These are *required invariants*, not proven library behavior yet. The existing
public `List<...>` members and constructor aliases mean the current Value
type does **not** by itself guarantee transitively immutable sharing. Until the
core borrow checker implements recursive ownership checks (and tests cover it),
do not use ordinary reference passing to hand mutable values between actors.

Each actor/supervisor has a single logical execution lane; separate actors may
execute in parallel. Mailbox handoff requires release/acquire publication
semantics (happens-before), not a literal JVM CPU-cache flush. Oreslang should
enforce ownership in its type system rather than depend on Scala/Akka-style
developer convention. Avoid exposing actor references, modules or classes as
serialized values; static members are not serializable instance capabilities.

## 4. Acceptance gates before release / merge

1. Native Oreslang compile and execution at the **exact PR head** on the
   corresponding exact compiler/runtime revision (not just a shell
   test-double runner).
2. JSON/MessagePack/CBOR unknown-payload nested arrays/maps and scalar roots,
   plus success and failure lookup paths.
3. Type metadata rejecting invalid UTF-8, empty names, duplicates and invalid
   field-number ranges; nested generated class/struct round trips.
4. Protobuf unknown field raw-byte preservation, nested typed messages, packed
   repeated, map/oneof, field presence and malformed lengths/varints.
5. Size/depth/string/container/output budgets and hostile/truncated input
   tests, incremental arbitrary chunk boundaries, duplicate key policies,
   integer/floating edge cases and UTF-8 correctness.
6. Actor-isolation compile-negative tests: mutable shared aliases rejected;
   `rt take` invalidates sender; `rt share` freezes all descendants;
   `rt borrow` cannot escape owner or unsafe `await`.
7. Build a real AOT target and inspect link closure: no Java codec libraries,
   reflection metadata, runtime dynamic class loading or JIT fallback.
8. If this org cannot admit Actions runners, mirror the **exact tree and blob
   SHAs** into a funded test org and execute native tests there without
   cross-organization secrets. Preserve evidence of exact-head equivalence.

Source-only tests and runner-contract workflows do not satisfy native or AOT
gates. Keep this work in a draft PR until the gates are verified.
