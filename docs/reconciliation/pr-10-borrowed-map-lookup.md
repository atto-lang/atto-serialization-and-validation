# PR #10 — recursive Value comparison and borrowed map lookup

Historical head 56fbc4a0f4bc1269218fe91d53c9baeddf4f5d35; current main 62e123bfa0d1f7a26578aa93df8dc56a80efc216.

The old branch replaced the common Value equality/hash representation with extensive borrowed parameters and `rt borrow` operations. Current main has newer dynamic Value constructors, shared structural transport and ownership/Task borrow contracts. Replacing current `src/core/value.ores`, JSON/CBOR/Protobuf/MessagePack modules or entire old tests would risk resurrecting obsolete source syntax and losing recent safeguards.

Preserve the semantic requirement: dynamic non-string MessagePack map lookup must compare **more than one key** without consuming, cloning or prematurely retiring the search Value; nested maps-of-maps/arrays must work, and duplicates obey the configured reject/first/last policy. The historical `tests/messagepack_test.ores` holds additional multiple-candidate and deeply nested map-key cases that should be forward-ported *after* verifying canonical `value_deep_equal` signature and ownership checking. They are not silently claimed passing on main.

**HOLD:** source-org compiler native execution, borrow checker tests, and Java/LLVM Value/ownership parity absent. This commit resolves historical ancestry with current main as implementation authority, but does not implement all borrowed-comparison refactoring or authorize a release.