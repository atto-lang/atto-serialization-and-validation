# Independent Protobuf diagnostic-envelope authorities

- `main.tsp`: authored TypeSpec describing a JSON-shaped inspection envelope.
- `authored.schema.json`: separately authored JSON Schema draft 2020-12.
- `instances/WireMessage/{valid,invalid}`: finite differential probes.
- `../../wit/protobuf-wire.wit`: unimplemented WIT adapter proposal.
- `../../src/protobuf/zorde.ores`: report-only bounded wire rule admission.
- `../../docs/PROTOBUF-WIRE-ZORDE-INTEROP.md`: full trust boundary and release gates.

Protobuf wire fields are not JSON objects; the envelope is a *projection*.
Reserved tag numbers are allowed syntactically as unknown wire data, forbidden
in trusted descriptor declarations. Semantic varint/fixed-width/packed
validation, Zorde compiler provenance, signedness, actor isolation, deep
immutability and exact byte budgets require native code and tests.

Do not generate either authority from the other and call it independent
validation. The pinned TypeSpec/JSON Schema parity workflow must actually
execute positive and adversarial negative comparisons; the WIT parser's
positive/negative tests are separate. Neither proves a running component.
