# Benchmark plan

Every format backend must run the same workload classes so regressions are comparable.

## Payloads

- tiny: <= 256 B
- small: 1-4 KiB
- medium: 64-256 KiB
- large: 1-16 MiB
- deeply nested adversarial payload
- wide object/map payload
- string-heavy payload
- numeric-heavy payload
- binary-heavy payload
- arrays of homogeneous records

## Operations

- encode typed
- decode typed
- decode dynamic materialized
- parse borrowed/tape view
- stream decode with 1 B, 7 B, 64 B, 1 KiB, and randomized chunk boundaries
- decode + fail-fast validation
- decode + collect validation
- validation only

## Metrics

- MB/s and values/s
- p50/p95/p99 latency
- allocations/value
- bytes allocated/value
- peak memory
- generated codec size
- AOT binary size delta

## External comparison targets

Where practical, compare equivalent schemas/payloads against:

- Rust serde_json, rmp-serde, prost
- Go sonic / encoding-json, msgpack, protobuf
- C++ simdjson / glaze, protobuf
- Java Micronaut Serialization, protobuf
- Kotlin kotlinx.serialization
- C# System.Text.Json source generation / MessagePack-CSharp

Benchmarks must document compiler version, CPU, architecture, flags, and whether validation is enabled.
