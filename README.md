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
