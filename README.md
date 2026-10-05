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
- [Validation model](docs/VALIDATION.md)
- [Ecosystem benchmark](docs/ECOSYSTEM-BENCHMARK.md)
- [Benchmark plan](benchmarks/README.md)
- [Proposed Oreslang surface](examples/proposed-api.md)


## Native implementation

The repository now contains a native Oreslang implementation on the `feat/native-codecs-validation-20261005` branch.

### Implemented

- `src/core/value.ores` — shared closed dynamic wire-value model.
- `src/core/options.ores` — bounded decode/encode policies and structured errors.
- `src/json/json.ores` — byte-oriented JSON encode/decode, Unicode escape handling, strict UTF-8 validation, duplicate-key policy, depth/size/container limits, and incremental chunk feeding.
- `src/messagepack/messagepack.ores` — native MessagePack integer/string/binary/array/string-map/extension encode/decode.
- `src/cbor/cbor.ores` — native definite-length CBOR integer/text/bytes/array/string-map/bool/null encode/decode.
- `src/protobuf/wire.ores` — native Protobuf field-key/varint/fixed32/fixed64/length-delimited wire encode/decode with unknown-field preservation.
- `src/validation/validation.ores` — reflection-free validation rules, bounded error collection, object schemas, type/length/item/numeric checks, ASCII/email/UUID/URL checks, uniqueness, unknown-field policy, and `ValidValue` typestate wrappers.
- native Oreslang tests under `tests/`.

The codecs use `List<int>` as the byte-buffer surface and retain dynamic text as validated UTF-8 bytes in `Utf8Text`. This keeps the implementation independent of JVM strings/reflection and makes it suitable for later zero-copy/native-buffer lowering.

### Pointer-free source model

This library follows the canonical Oreslang reference model:

- ordinary objects are reference values and ordinary arguments pass references;
- source code does not use `&T`, `&value`, `T*`, unary dereference `*value`, or pointer arithmetic;
- mutation authority uses `Type mut name`;
- ownership changes use `rt copy`, `rt borrow`, `rt take`, and `rt share` when needed.

Raw addresses, SIMD pointers, device pointers, and similar low-level details belong below the Oreslang source boundary in compiler/runtime backends.

### Core integration prerequisites

The implementation targets the intended combined Oreslang language/runtime surface currently represented by:

- core PR **#135**: native-backed `List`/`Map` values, typed structs, and serialization annotation infrastructure;
- core PR **#186**: Java-like reference passing, canonical `Type mut name`, and explicit `rt` ownership operations.

Those capabilities are still on separate core branches, so this repository should remain draft-gated until that core surface is reconciled.

### Deliberate fail-closed baseline

Exact wire semantics are preferred over lossy coercion. The current baseline therefore rejects features whose exact primitive/runtime representation has not landed yet:

- MessagePack float32/float64 and 64-bit integer markers;
- CBOR float16/32/64, tags, indefinite-length items, and uint64 additional arguments;
- raw Protobuf varints larger than signed Oreslang `int` range and deprecated group wire types.

These cases return explicit codec errors rather than silently narrowing values. Generated typed codec support can extend them as exact unsigned-64 and float-bit primitives become available.

## Running native tests

```sh
ORESLANG_BIN=/path/to/oreslang ./scripts/test.sh
```

The test program exercises JSON, incremental JSON, UTF-8 rejection, duplicate-key policy, MessagePack, CBOR, Protobuf wire round-trips, and validation.
