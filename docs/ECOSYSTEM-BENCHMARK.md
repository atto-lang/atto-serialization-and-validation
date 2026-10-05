# Ecosystem benchmark

The goal is not to copy syntax. It is to extract the best architecture from mature AOT/static ecosystems.

## Rust

- JSON: Serde + serde_json; simd-json / sonic-rs for specialized high-throughput paths.
- MessagePack: rmp-serde.
- Protobuf: prost.
- Validation: Garde (and validator as another common derive-based option).

What to borrow: derive/codegen, zero-cost traits, borrowed deserialization, typed + untyped JSON, streaming deserializers, validation derives, typestate for validated values.

## Go

- JSON: encoding/json for compatibility; sonic for high-throughput specialized JSON.
- MessagePack: vmihailenco/msgpack.
- Protobuf: google.golang.org/protobuf; vtprotobuf-style generated fast paths.
- Validation: go-playground/validator.

What to borrow: simple tag-driven field metadata, generated Protobuf dispatch, pooled buffers, explicit streaming Decoder APIs.

## C++20/23

- JSON: simdjson for parsing; glaze for compile-time reflection and multi-format typed serialization; nlohmann/json for ecosystem ubiquity.
- MessagePack: glaze / msgpack-cxx.
- Protobuf: google::protobuf.
- Validation: JSON Schema validators plus compile-time/domain validation.

What to borrow: SIMD structural scans, on-demand/tape views, compile-time field metadata, allocator control.

## Swift

- JSON: Codable + JSONDecoder/JSONEncoder.
- MessagePack: Codable-integrated MessagePack libraries.
- Protobuf: SwiftProtobuf.
- Validation: server-side Validation libraries and macro/property-wrapper based rules.

What to borrow: one language-level coding model with custom CodingKeys and generated conformance.

## Zig

- JSON: std.json.
- MessagePack: lightweight Zig MessagePack implementations.
- Protobuf: generated Zig protobuf libraries.
- Validation: comptime-generated explicit validators.

What to borrow: comptime specialization, explicit allocators, parser limits, no hidden reflection.

## C# NativeAOT

- JSON: System.Text.Json source generation.
- MessagePack: MessagePack-CSharp source generators.
- Protobuf: Google.Protobuf / protobuf-net generated code.
- Validation: DataAnnotations / FluentValidation.

What to borrow: source-generated serialization contexts and AOT-safe metadata.

## Java + GraalVM Native Image

- JSON: Micronaut Serialization is the most directly relevant design: build-time introspection, reflection-free runtime, Jackson-compatible annotation model.
- MessagePack: msgpack-core / Jackson MessagePack format integrations.
- Protobuf: protobuf-java generated messages.
- Validation: Jakarta Validation / Hibernate Validator, with AOT-aware framework integration.

What to borrow: compile-time bean metadata and a narrow runtime with no arbitrary-object reflection.

## Kotlin/Native

- JSON: kotlinx.serialization.
- MessagePack: kotlinx.serialization MessagePack formats such as kotlinx-serialization-msgpack.
- Protobuf: kotlinx-serialization-protobuf or generated protobuf bindings.
- Validation: Konform-style typed DSLs / generated custom validators.

What to borrow: compiler-plugin generated serializers, one Encoder/Decoder abstraction shared across formats, reflectionless multiplatform behavior.

## Dart AOT

- JSON: json_serializable.
- MessagePack: messagepack streaming APIs; generator-based MessagePack serializers also exist.
- Protobuf: google/protobuf.dart generated messages.
- Validation: no single serialization-coupled validator dominates; generated/model-specific validation is preferable for Oreslang.

What to borrow: build_runner-like generation, generated fromJson/toJson, stream-oriented binary unpackers.

## Nim

- JSON: jsony for direct-to-object speed; status-im/nim-json-serialization for strict and configurable typed/dynamic parsing.
- MessagePack / Protobuf: nim-serialization family and protobuf-serialization projects.
- Validation: no single dominant validation framework; compile-time macros/custom generated validators fit the language well.

What to borrow: direct-to-object parsing that avoids an intermediate DOM, compile-time customization hooks, raw JSON passthrough.

## Oreslang synthesis

The common high-performance pattern is:

1. compiler/build-generated metadata;
2. a generic Encoder/Decoder contract;
3. direct typed decode without intermediate maps;
4. dynamic DOM/map support when schema is unknown;
5. borrowed/on-demand views for extreme throughput;
6. incremental parser state machines;
7. validation compiled next to decode;
8. explicit limits and unknown/duplicate-field policies.

That is the design target for this repository.
