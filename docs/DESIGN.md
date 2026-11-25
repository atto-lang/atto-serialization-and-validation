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
- string-key object
- general map with dynamic keys
- extension value (MessagePack)
- raw/unknown field bytes where a format can preserve them

JSON objects use string keys. MessagePack and CBOR maps preserve non-string keys in the general-map `Value` kind; maps whose keys are all UTF-8 text materialize as the string-key object kind for convenient record-shaped APIs.

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

- `@Required`
- `@MinLength(n)`
- `@MaxLength(n)`
- `@Length(min, max)`
- `@Min(n)`, `@Max(n)`, `@Range(min, max)`
- `@MinItems(n)`, `@MaxItems(n)`
- `@Pattern("...")`
- `@Email`, `@Uuid`, `@Url`
- `@OneOf(...)`
- `@ValidateNested`
- `@CustomValidator(name)`
- `@WireName("external_name")`
- `@FieldNumber(n)` for Protobuf/codegen interop
- `@UnknownFields(ignore|reject|preserve)`

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

CBOR follows the MessagePack-style binary value path. The current baseline implements definite-length integers, text, bytes, arrays, maps, booleans, and null; tags, floats, and indefinite-length items are rejected explicitly until their exact representations and streaming contracts land.

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


## Implementation status

The native implementation uses a single dynamic `Value` contract across formats. Dynamic object values are represented as an ordered `List<WireMember>` rather than immediately collapsing into a map. This is intentional:

- JSON/CBOR/MessagePack duplicate-key policy can be enforced deterministically;
- source/wire order can be preserved;
- field names stay as validated UTF-8 bytes rather than requiring host/JVM strings;
- generated typed decoders can dispatch directly without first allocating a `Map`.

Callers that need a map-shaped view can materialize one after duplicate/unknown-field policy is resolved.

### Current native layers

```text
List<int> wire bytes
       |
       +--> JSON --------+
       +--> MessagePack -+--> Value --> validation --> ValidValue
       +--> CBOR --------+
       |
       +--> Protobuf wire fields --> generated/schema-aware typed decoder
```

JSON is a byte-oriented parser with strict UTF-8 and Unicode escape handling. The incremental JSON decoder currently retains chunks and re-runs the bounded parser over the accumulated buffer; this establishes correct arbitrary-boundary semantics first. A later optimization can retain parser state/tape positions between feeds without changing the public result contract.

MessagePack and CBOR share the dynamic `Value` model while preserving binary-vs-text distinctions. Protobuf deliberately exposes numbered `ProtoField` values because a schema-less Protobuf payload does not have meaningful field names.

### Typed generated codecs

The native dynamic/wire layers are implemented. Fully generated direct-to-class/struct codecs remain a compiler integration layer: compiler metadata from `@FromJson`, `@WireName`, `@FieldNumber`, validation annotations, TypeSpec, or JSON Schema should generate ordinary Oreslang decode/encode functions that call these wire primitives. They must not use runtime reflection.

This separation is deliberate: generic wire correctness belongs in this repository; static field discovery and ABI metadata belong in the Oreslang compiler/build pipeline.
