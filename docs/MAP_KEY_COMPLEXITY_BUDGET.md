# Dynamic map-key complexity budget

JSON objects and MessagePack maps use hash buckets to avoid comparing every key with every prior key. Input byte and entry limits bound allocation and recursion, but a hash bucket can still contain many candidates. Repeated deep equality on a collision chain can otherwise become quadratic.

## Decoder budget

Each top-level JSON or MessagePack decode owns one mutable `map_key_work_remaining` counter in its cursor. Recursive object/map decoding receives the same cursor, so nested maps share one total allowance. The counter is never process-global and is discarded with the operation.

The initial allowance is derived from the already validated limits:

`min(8,000,000, 256 + min(max_map_entries, 1,000,000) * 8)`

The helper caps `max_map_entries` before multiplication. The byte/input limit still bounds one-pass hashing of each input key. The additional comparison budget charges repeated hash-bucket candidates. The helper uses integer-only arithmetic; it does not rely on implicit integer division or floating-point conversion.

JSON charges one unit plus both key byte lengths before `bytes_equal`. MessagePack computes a bounded structural work estimate for each dynamic key (nodes plus string/number/binary payload bytes, recursively through arrays, objects and general maps). Before deep equality it charges the estimate pass and equality pass for both candidate keys. The estimator stops at the remaining budget and respects the same maximum depth as dynamic key equality.

When a candidate comparison cannot be admitted, decoding fails with structured `DecodeError.code = "map_complexity_limit"`. It never skips equality, accepts a possible duplicate, changes `reject`/`first_wins`/`last_wins`, or publishes a partial decoded value.

## Encoder budget

Each top-level JSON/MessagePack writer owns one shared collision counter and one key-hash work counter. The hash-work counter starts at `max_output_bytes`, bounding preflight of a caller-supplied in-memory key before the encoder has emitted bytes. The encoder collision counter uses the bounded one-million-entry ceiling; the separate key-hash counter uses `max_output_bytes`. Nested maps share both counters through the writer.

Duplicate keys still produce `duplicate_key` when comparison work is admitted. If the work allowance is exhausted first, encoding fails with `map_complexity_limit`; it never emits an ambiguous map.

## Tests and evidence

JSON tests use repeated 60-byte string keys with `first_wins`: two candidate comparisons fit the chosen work allowance, while the fourth candidate comparison returns `map_complexity_limit` through `DecodeAttempt`.

MessagePack tests use repeated 16-byte string keys: a four-entry map fits; a seven-entry map exceeds the budget. A separate array containing six individually small nested maps exceeds the same total allowance, proving that nested decoders share one cursor counter instead of receiving fresh per-map budgets.

The code is AOT-friendly and uses operation-local mutable cursor/writer state. It requires native `.ores` compiler execution on Java/GraalVM and LLVM before promotion; source review or host tests alone are not acceptance.