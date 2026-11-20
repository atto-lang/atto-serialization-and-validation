# PR #9 post-merge codec integrity reconciliation

Historical head `818729b768ae08c7da9140f13a824b1cf93f4d68`; current main `62e123bfa0d1f7a26578aa93df8dc56a80efc216`.

Current main has independently evolved bounded JSON/CBOR/MessagePack paths, reviewed borrow forms, metadata and schema codec types, and a newer no-Python repository policy. We do not overwrite its dynamic Value, import graph, or current .push/collection API with the historical snapshot. The old `scripts/audit.py` is deliberately **not** restored.

Unique historical signed-decimal regression was forward-ported into current `tests/json_test.ores`: zero, negative seventeen, INT64_MAX and INT64_MIN must serialize into precise ASCII digit bytes. It exercises the modern `int_to_decimal_bytes` implementation against its original wire-level oracle without relaxing current codec limits.

Scope is these four assertions, **not** an assertion that all 15 obsolete PR files or the full original feature are now implemented. Execute exact-head Oreslang native tests and validate borrow/transitive aliasing, JSON, MessagePack and schema exports before merge; this PR must remain draft until CI and review clear.
