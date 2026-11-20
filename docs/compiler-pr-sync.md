# Oreslang compiler PR cross-repo checkpoint (2026-10-06)

**Repository:** `oreslang-serialization-and-validation` — Serialization / validation wire formats.

**Status at review:** The 10 newest compiler PRs were all open, not merged into `oreslang-source.java/main`. This PR documents interoperability and gates only; it does not claim those language/runtime changes are implemented in this downstream repository.

## Source-of-truth PRs
- [oreslang-source.java #398](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/398)
- [oreslang-source.java #399](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/399)
- [oreslang-source.java #404](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/404)
- [oreslang-source.java #405](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/405)
- [oreslang-source.java #406](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/406)
- [oreslang-source.java #408](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/408)
- [oreslang-source.java #409](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/409)
- [oreslang-source.java #410](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/410)
- [oreslang-source.java #411](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/411)
- [oreslang-source.java #412](https://github.com/ores-truffle-oreslang/oreslang-source.java/pull/412)

The select dependency stack is **#405 -> #406 -> #408 -> #412**; #404 is stacked on a match feature branch; #398, #399, #409, #410 and #411 are separate work. Never treat any one PR as the full compiler feature set.

## oreslang-serialization-and-validation migration invariants
- MessagePack, Protobuf, CBOR and JSON bytes remain data-only; never serialize live functions, raw references, borrows or `rt proxy` locks (#399/#410/#411).
- Use nested `Option<Option<T>>` or explicit presence metadata where a map can distinguish missing key, present None and present Some(value) (#398/#404).
- For proposed streaming APIs, ordinary local channels may carry data-only typed records; crossing actor mailboxes requires serialization/freeze (#399/#410).
- Preserve exact error and byte-for-byte unknown-field round trips across all existing formats; do not rebind readonly buffers with new qualifier spelling.

## Conformance checks
- ORESLANG_BIN=/path/to/oreslang ./scripts/test.sh
- Negative validation for callable payload and untrusted alias transport
- Fuzz malformed and truncated wire payloads after compiler update

## Version policy

Keep existing `compiler.lock`, `SOURCE_REF`, config and CI default behavior unchanged where present. For new features, test against an **exact commit SHA** of a compiler branch incorporating all required dependencies, then move pins only once that build is independently verified. A GitHub Action failure with `runner_id=0` and no steps is **not** a failed test or a passed test; mirror the exact Git tree to a funded organization without sharing credentials and verify blob/tree SHAs before claiming evidence.

No `&` or `*` pointer syntax should be added to public Oreslang code: use `rt borrow`, `rt copy`, `rt share`, `rt take` and capability-checked `rt proxy` where supported. Actor mailboxes carry serialized/frozen data, never function pointers or mutable object references.
