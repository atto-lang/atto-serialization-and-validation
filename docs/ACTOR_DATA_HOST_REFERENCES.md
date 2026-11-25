# Actor DATA host reference models

Status: Rust host references only. These are not a production AttoLang library, compiler registration, guest runtime behavior, authenticated transport, or AOT/conformance certification. Native implementation must be written in canonical Atto source and exercised through compiled guest tests.

The models are deliberately independent of the external network codec boundary. Network codecs cannot establish actor admission or supervisor CONTROL authority. A future compiler-derived actor DATA codec needs a trusted static binding and recipient generation, a bounded decoder, separately owned receiver storage, and transactional admission. No guest-defined codec may authorize CONTROL, transport live handles or borrow identities, or grant sendability by itself.

| Reference | Scope |
| --- | --- |
| `tools/actor_data_codec_policy_v1.rs` | Trusted static codec selection and exact metadata matching; no authentication or wire parsing. |
| `tools/actor_data_decode_budget_v1.rs` | Checked cumulative logical decoder accounting; no physical allocator proof. |
| `tools/actor_data_mailbox_credits_v1.rs` | Generation-fenced DATA queue credit and reservation reference; no actual actor runtime. |
| `tools/actor_data_cbor_preflight_v1.rs` | Restricted deterministic typed CBOR profile, bounded preflight/decoding/encoding; not general CBOR. |

Each file is a standalone Rust test target, built with `rustc --edition=2021 -D warnings --test`. CI has separate focused workflows. A passing Rust reference test is not a passing Atto guest test.

Legacy proposal preservation: the earlier full draft snapshot is retained at commit `7be82118534913252d9a98a894a6c514c99bf466` (branch `recovery/dual-serde-legacy-full-snapshot-20261125`). Its proposed transport signatures, static guards, and distinct network/actor ownership design require a separate current-language compiler-backed integration. The legacy decimal converter used division; current main already has exact signed-int64 conversion without floating-point division and remains authoritative. No native implementation or regression is replaced by this host-reference stage.
