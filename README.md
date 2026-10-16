# oreslang-serialization-and-validation

High-performance serialization, deserialization, streaming decode, and payload validation for Oreslang.

## Goals

- AOT-friendly and reflection-free typed codecs.
- Fast dynamic decoding for unknown payloads.
- Generated/specialized decoding for recognized classes and structs.
- Incremental and streaming decode APIs.
- One validation model shared by JSON, MessagePack, Protobuf, and future formats.
- Bounded parsing with explicit depth/size/container limits.
- Zero-copy/borrowed views where lifetime and ownership rules allow them.
- Deterministic behavior for duplicate keys, unknown fields, numeric coercions, and malformed input.

## Initial formats

1. JSON
2. MessagePack
3. Protocol Buffers
4. CBOR

The public API should stay format-neutral where possible while preserving format-specific capabilities such as MessagePack extension values and Protobuf field numbers / unknown fields.

## Two distinct serialization interfaces

The source declarations in [`src/schema/transport_interfaces.ores`](src/schema/transport_interfaces.ores)
define `NetworkCodec<T, F>` (explicit JSON/MessagePack/Protobuf format) for external HTTP/TCP/WebSocket/RPC/persistence
payloads and `ActorMessageCodec<T>` for supervisor-to-actor and actor-to-actor
**DATA** messages. They share primitive codec machinery but never implicitly
grant one another's permissions. Supervisor `CONTROL` (including
`on_panic`/`on_raise`/`on_throw`) remains a sealed runtime protocol.

Both APIs synchronously **borrow** input while encoding, return **owned**
output bytes, and decode into a distinct **owned** value. Borrowed input
views, `Pointer<T>`, closures, stream/channel handles and actor capabilities
cannot be serialized or smuggled through generic containers. An actor send
requires an atomic reserve/encode/commit boundary with rollback on failure;
this part remains a compiler/runtime integration gate, not an enabled
userland feature. `Sendable<T>` is compiler-verified data/ownership eligibility,
not a user-implementable trait or a synonym for serializable.
See [the two-domain ownership contract](docs/NETWORK_VS_ACTOR_SERDE.md)
and [the Sendable vs Serializable format model](docs/SENDABLE_VS_SERIALIZABLE_AND_WIRE_FORMATS.md).

## Architecture

The implementation is intentionally split into:

- **dynamic value decoding** for unknown payloads (`Map<String, Value>`, arrays, scalars);
- **typed generated codecs** for known Oreslang classes/structs;
- **token/cursor decoding** for low-allocation traversal;
- **incremental streaming decoders** for chunked input;
- **compiled validation** derived from field/type annotations and contract metadata.

See the design documents on the feature branch for the detailed API and performance model.

## Design documents

- [Architecture](docs/DESIGN.md)
- [Std-lib/core migration and format hardening](docs/STDLIB_MIGRATION.md)
- [Format-specific semantic models](docs/FORMAT_MODELS.md)
- [Validation model](docs/VALIDATION.md)
- [Ecosystem benchmark](docs/ECOSYSTEM-BENCHMARK.md)
- [Benchmark plan](benchmarks/README.md)
- [Proposed Oreslang surface](examples/proposed-api.md)


## Native implementation

The repository now contains the native Oreslang implementation on `main`; this hardening branch adds the first built-in channel/async adapter and the std-lib/core migration contract.

### Implemented

- `src/core/value.ores` — shared closed dynamic wire-value model.
- `src/core/options.ores` — bounded decode/encode policies and structured errors.
- `src/json/json.ores` — byte-oriented JSON encode/decode, Unicode escape handling, strict UTF-8 validation, duplicate-key policy, depth/size/container limits, and incremental chunk feeding.
- `src/json/channel.ores` — native `Future`/`async` channel adapter using `nb readch`; only byte data crosses the channel boundary.
- `src/messagepack/messagepack.ores` — native MessagePack signed-int64/bounded-uint64/string/binary/array/general-map/extension encode/decode.
- `src/cbor/cbor.ores` — native definite-length CBOR signed-int64/bounded-uint64/text/bytes/array/general-map/bool/null encode/decode.
- `src/protobuf/wire.ores` — native Protobuf field-key/varint/fixed32/fixed64/length-delimited wire encode/decode with exact raw uint64-varint unknown-field preservation.
- `src/validation/validation.ores` — reflection-free validation rules, bounded error collection, object schemas, type/length/item/numeric checks, ASCII/email/UUID/URL checks, uniqueness, unknown-field policy, and `ValidValue` typestate wrappers.
- `src/schema/metadata.ores` + `src/schema/object_codec.ores` — compiler-generated static field metadata and metadata-driven validated JSON object projection.
- native Oreslang tests under `tests/`.

The codecs use `List<int>` as the byte-buffer surface and retain dynamic text as validated UTF-8 bytes in `Utf8Text`. This keeps the implementation independent of JVM strings/reflection and makes it suitable for later zero-copy/native-buffer lowering.

### Pointer-free source model

This library follows the canonical Oreslang reference model:

- ordinary objects are reference values and ordinary arguments pass references;
- source code does not use `&T`, `&value`, `T*`, unary dereference `*value`, or pointer arithmetic;
- mutation authority uses `Type mut name`;
- ownership changes use `rt copy`, `rt borrow`, `rt take`, and `rt share` when needed.

Raw addresses, SIMD pointers, device pointers, and similar low-level details belong below the Oreslang source boundary in compiler/runtime backends.

### Core integration baseline

The library targets current pointer-free Oreslang semantics: ordinary calls use managed-reference behavior and explicit ownership changes use compiler-owned `rt borrow`, `rt take`, `rt copy`, and `rt share`. The pointerless ownership convergence from compiler PR **#381** is now on current Oreslang `main`.

Streaming adapters use the built-in `Channel<T>`, native `Future<T>`, and `async`/`await` surface rather than introducing a second scheduler or callback transport. Channel payloads remain ordinary data; parser state and callbacks stay local to the decoding execution domain.

Promotion into core/std-lib is still gated on format-specific conformance, hostile-input, streaming-boundary, and ownership tests. XML, YAML, and TOML are intentionally not presented as implemented until bounded native parsers exist; see [the migration and hardening plan](docs/STDLIB_MIGRATION.md).

### Deliberate fail-closed baseline

Exact wire semantics are preferred over lossy coercion. The current baseline therefore rejects features whose exact primitive/runtime representation has not landed yet:

- MessagePack float32/float64 markers and uint64 values above signed Oreslang `int` when materialization is requested;
- CBOR float16/32/64, tags, indefinite-length items, and uint64 values above signed Oreslang `int`;
- deprecated Protobuf group wire types.

Protobuf wire-type-0 values above signed `int` are still accepted and preserved as exact raw varint bytes so unknown fields round-trip without narrowing.

These cases return explicit codec errors rather than silently narrowing values. Generated typed codec support can extend them as exact unsigned-64 and float-bit primitives become available.

## Running native tests

```sh
ORESLANG_BIN=/path/to/oreslang ./scripts/test.sh
```

The test program exercises JSON, incremental JSON, UTF-8 rejection, duplicate-key policy, MessagePack, CBOR, Protobuf wire round-trips, and validation.

## Compiler compatibility checkpoint (2026-10-06)

This repository tracks the latest 10 compiler PRs as **unmerged proposals**, without changing its stable compiler pin or API behavior. See [docs/compiler-pr-sync.md](docs/compiler-pr-sync.md) for oreslang-serialization-and-validation-specific migration invariants, dependencies and validation gates.

## Runner CI and native-suite coverage

The `Test runner contracts` workflow checks shell syntax and uses a compiler
test double to prove repository-root execution, paths containing spaces,
explicit compiler selection, PATH fallback, exact output/exit-code propagation,
and failure when the compiler is missing. It uses read-only permissions and no
secrets. The runner now works when launched outside this repository.

This is a launcher gate, not codec conformance evidence. A 2026-10-07 probe
against compiler `5ffc52d9297d175307094d530293c69f1cafc7cd` stopped at the
unexported `Value` class imported from `src/core/value.ores`. Resolve public
exports and subsequent ownership diagnostics, then validate all codec fixtures
before selecting a supported compiler pin and enabling a native-suite CI job.


## Dynamic nested payloads and release status

JSON, MessagePack and CBOR decode unknown-schema payloads to a recursive
`Value` tree. Object fields are ordered `WireMember` entries, while
MessagePack/CBOR non-string-key maps are `MapEntry` entries; these are **not**
yet native `Map<String, Value>` materializations. Native `value_lookup` and
`value_lookup_path` allow bounded traversal across any mixture of objects,
arrays and general maps without host-language reflection.

This source design is **AOT-compatible**, but full native/AOT end-to-end
compilation, compiler-generated class/struct codecs and transitive actor-safe
sharing remain separate release gates. In particular, mutable nested
`List` fields cannot be safely shared across actor lanes until recursive
`rt share` / `rt take` / `rt copy` / `rt borrow` semantics are enforced
by the compiler. See [the ownership and AOT contract](docs/DYNAMIC-AOT-OWNERSHIP.md).
