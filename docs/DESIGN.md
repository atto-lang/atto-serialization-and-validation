# Serialization and validation design

## Principles

1. **No runtime reflection on the hot path.** Known types get generated/specialized codecs at build time.
2. **Unknown payloads remain first-class.** JSON and MessagePack can decode directly into a dynamic value tree backed by Oreslang maps/arrays.
3. **Streaming is not a separate format.** The same decoder state machine accepts a complete byte slice, a pull source, or incrementally fed chunks.
4. **Validation compiles into typed decoders.** Required fields, lengths, ranges, patterns, nested rules, and custom validators should normally be checked while the object is being constructed.
5. **Safety limits are mandatory.** Every decoder has bounded nesting, input bytes, string/binary length, map entries, array entries, and unknown-field storage.
6. **AOT first.** Metadata needed for codecs and validators is emitted by the compiler/build step; no classpath scanning or reflective field discovery is required.

## Core data model

Dynamic payloads use a closed tagged value model:

- null
- bool
- signed integer
- unsigned integer
- floating point
- string
- bytes
- array
- map
- extension value (MessagePack)
- raw/unknown field bytes where a format can preserve them

JSON maps use string keys. MessagePack dynamic maps may use any scalar key internally, with a convenience strict-string-map mode for JSON-shaped APIs.

For maximum throughput, the library should expose both:

- **materialized dynamic decode**: build `Map<String, Value>` / arrays;
- **borrowed view/tape decode**: retain offsets into immutable input and allocate only when the caller materializes a value.

## Typed codecs

Known classes/structs get a generated decoder containing:

- precomputed field-name hashes for JSON/MessagePack string-key maps;
- precomputed Protobuf field-number dispatch;
- direct writes into constructor slots / fields;
- a required-field bitset;
- unknown-field policy;
- validation checks emitted adjacent to decode logic;
- no intermediate dynamic map.

The generated decoder should specialize common scalar conversions and avoid allocating temporary field-name strings when the source buffer can be compared directly.

## Public API shape

Conceptually:

```text
encode<T>(format, T value, EncodeOptions options = ...)
decode<T>(format, Bytes input, DecodeOptions options = ...) -> T
decode_dynamic(format, Bytes input, DecodeOptions options = ...) -> Value
decode_map(format, Bytes input, DecodeOptions options = ...) -> Map<String, Value>
parse_view(format, Bytes input, DecodeOptions options = ...) -> ValueView

decoder<T>(format, DecodeOptions options = ...) -> IncrementalDecoder<T>
dynamic_decoder(format, DecodeOptions options = ...) -> IncrementalDecoder<Value>
```

Streaming decoder state:

```text
NeedMore
ValueReady<T>
Error<DecodeError>
```

A decoder accepts arbitrary chunk boundaries, including boundaries inside UTF-8 sequences, JSON escape sequences, MessagePack length prefixes, and Protobuf varints.

## Streaming modes

Support all of these without duplicating parsers:

- `decode_from(reader)`
- `decode_from_async(Stream<Bytes>)`
- `decoder.feed(chunk)`
- token/cursor iteration
- NDJSON / JSON Lines sequence decoding
- length-delimited Protobuf message sequences
- concatenated MessagePack object sequences

A completed value must not consume bytes belonging to the next value. The decoder reports consumed byte count or retains the unconsumed suffix.

## Validation during decode

Typed decode defaults to **decode + validate in one pass**.

Two error modes:

- **fail_fast**: stop at the first invalid field;
- **collect**: aggregate validation errors up to a configured maximum.

Validation metadata is generated from Oreslang annotations/attributes and may also be generated from JSON Schema / TypeSpec contracts.

## Validation annotations

Initial field/type vocabulary:

- `@required`
- `@min_length(n)`
- `@max_length(n)`
- `@length(min, max)`
- `@min(n)`, `@max(n)`, `@range(min, max)`
- `@min_items(n)`, `@max_items(n)`
- `@pattern("...")`
- `@email`, `@uuid`, `@url`
- `@one_of(...)`
- `@validate_nested`
- `@custom_validator(name)`
- `@wire_name("external_name")`
- `@field_number(n)` for Protobuf/codegen interop
- `@unknown_fields(ignore|reject|preserve)`

Compiler spelling can evolve, but all forms should lower to one static metadata representation.

## Required versus presence

Do not conflate language nullability, wire presence, and semantic validation:

- **nullable**: the value may be null;
- **optional/present**: the field may be absent on the wire;
- **required**: validation requires it to be present;
- **default**: absence may synthesize a value.

This distinction matters especially for Protobuf proto2/proto3 and PATCH-like APIs.

## Decode options

Important policies:

- unknown fields: ignore / reject / preserve;
- duplicate keys: reject / first_wins / last_wins;
- numeric coercion: strict by default;
- invalid UTF-8: reject by default;
- trailing data: reject / allow / sequence;
- max depth;
- max input bytes;
- max string/binary bytes;
- max map entries;
- max array entries;
- max validation errors.

## Format notes

### JSON

Implement a hand-written byte-oriented state machine. Typed decoding should compare field names directly against UTF-8 input and only allocate strings for actual string values that cannot be borrowed.

Later optimization tiers may use SIMD for structural character discovery while retaining the same decoder contract.

### MessagePack

Use the same dynamic `Value` model, preserve extension values, and choose the smallest legal representation when encoding. Support both map-shaped typed records and compact positional layouts as an opt-in contract.

### Protobuf

Typed/generated decoding is the primary mode. Preserve unknown fields when requested. Support proto2 presence semantics, proto3 optional/presence, packed repeated fields, maps, oneofs, and length-delimited streaming messages.

Dynamic Protobuf decoding requires a descriptor/schema; raw unknown Protobuf bytes without a descriptor should not pretend to have JSON-like field names.

### CBOR

CBOR follows the MessagePack-style binary value path and is the fourth initial backend because it exercises tags, binary strings, integers, and indefinite/streamed containers.

## Performance model

Benchmark separately:

1. bytes -> typed object;
2. bytes -> materialized dynamic map/value;
3. bytes -> borrowed view/tape;
4. typed object -> bytes;
5. incremental chunked decode;
6. validation-only;
7. decode+validation fused.

Track throughput, allocations/value, peak resident bytes, and tail latency.

## Security

All parsers must be non-recursive or enforce a hard depth bound, reject integer/length overflows, cap allocations before reserving memory, and never trust declared container/string lengths before bounds checks.

Unknown/untrusted actors must receive only ordinary Oreslang values/handles from this library; codec internals or host VM objects must never escape into guest code.
