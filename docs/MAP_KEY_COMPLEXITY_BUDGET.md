# Dynamic map-key complexity budget

JSON objects and MessagePack maps use hash buckets to avoid comparing every key with every prior key. Input byte and entry limits bound allocation and recursion, but a hash bucket can still contain many candidates. Repeated deep equality on a collision chain can otherwise become quadratic.

## Decoder budget

Each top-level JSON or MessagePack decode owns one mutable `map_key_work_remaining` counter in its cursor. Recursive object/map decoding receives the same cursor, so nested maps share one total allowance. The counter is operation-local, never process-global.

The allowance is `min(8,000,000, 256 + min(max_map_entries, 1,000,000) * 8)`. Arithmetic is integer-only. The serialized input byte limit already bounds one-pass decoder hashing; the additional budget bounds repeated hash-bucket candidate equality.

JSON charges one unit plus both key byte lengths before comparing a candidate. MessagePack computes a bounded structural work estimate for dynamic keys (nodes plus string/number/binary payload bytes, recursively through arrays, objects and general maps). For nested object/general-map keys, it also charges the square of the nested entry count multiplied by the aggregate bounded child structural cost. This covers the equality helper's worst-case candidate scan when each candidate comparison itself traverses nested keys or values. Before deep equality it charges the estimate and equality work for both candidate keys. The estimator stops at the remaining budget and rejects keys deeper than the configured `max_depth` before recursive equality.

When a candidate comparison cannot be admitted, decoding fails with structured `DecodeError.code = "map_complexity_limit"`. It never skips equality, accepts a possible duplicate, changes `reject`/`first_wins`/`last_wins`, or publishes a partial decoded value.

## Encoder budget

Each top-level JSON/MessagePack writer owns one shared collision counter and one key-hash work counter. The hash-work counter is `min(max_output_bytes, 8,000,000)`, bounding preflight of caller-owned keys before recursive hashing. The collision counter uses the fixed eight-million-entry ceiling. Encoder map admission is capped at one million entries, matching the decoder's supported container limit. Nested maps share both counters through the writer.

Duplicate keys still produce `duplicate_key` when comparison work is admitted. If the work allowance is exhausted first, encoding fails with `map_complexity_limit`; it never emits an ambiguous map.

## Tests and evidence

JSON tests use repeated 60-byte keys with `first_wins`: two candidate comparisons fit; the fourth candidate comparison exceeds the budget. A nested-array test proves objects share the same cursor budget, and an encoder test checks a caller-owned key larger than `max_output_bytes`.

MessagePack tests use repeated 16-byte keys: a four-entry map fits; a seven-entry map exceeds the budget. Six individually cheap nested maps collectively exceed the same decoder-wide allowance. Encoder tests cover caller-owned key hashing and a nested key beyond configured `max_depth`. A decoder fixture embeds equal 16-entry general maps with duplicate/colliding inner keys as outer keys to verify nested equality scan is rejected before quadratic work.

The code uses operation-local cursor/writer state and no reflection. Native `.ores` compilation and guest execution on Java/GraalVM and LLVM are still required before promotion; source review and host-only tests are not native codec evidence.