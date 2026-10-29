# Zorde compiler-owned constrained aliases — report-only v1 adapter

**Status: experimental guest source; no compiler-executed evidence and no
authoritative `Valid<T>` construction. This file does not authorize a merge.**

## Interoperability

The Java/Graal compiler's
`dev.oreslang.compiler.TypeConstraintDescriptors.Catalog` and LLVM's
`contracts/v1/type-alias-extension-constraint-v1.properties` define
`ores.type-constraint.v1` for *direct, nongeneric signed i64 aliases*.
Only `min_inclusive_i64` and `max_inclusive_i64` are currently supported.
The source `declare type Code = int with min 0 and max 5;` generates a
qualified alias identity, canonical predicate order, mandatory
`validationRequired=true`, stable deterministic error IDs and the SHA-256
fingerprint of the compiler-owned canonical descriptor. A public Rust/Java
or Oreslang dictionary carrying the same string fields is **not** a trusted
compiler catalog.

The new native
[`src/zorde/compiler_constraint_v1.ores`](../src/zorde/compiler_constraint_v1.ores)
accepts a *projection* of the compiler's v1 catalog for **ordinary report-only
value checking**. It has two entrypoints:

- `zorde_check_compiler_i64_v1(value, descriptor, expected_name, expected_fingerprint)`
  returns a `ValidationReport` only.
- `zorde_parse_compiler_i64_v1(...)` returns
  `Result<int, ValidationReport>`. The integer is plain `int` and is
  deliberately not a nominal constrained alias or a proof.

Both caller-supplied `expected_name` and `expected_fingerprint` are data.
They must ultimately originate from a compiler-verified package/loader catalog,
with a separately authenticated artifact origin. Comparing 64 lowercase
hexadecimal characters alone does not compute SHA-256, verify the canonical
predicate bytes, enforce authority or prevent spoofing.

**Reference compiler v1 golden:** qualified type `Code`, representation
`i64`, inclusive minimum `0`, inclusive maximum `5`, error IDs
`ORES_CONSTRAINT_MIN_INCLUSIVE_I64` and
`ORES_CONSTRAINT_MAX_INCLUSIVE_I64`, SHA-256
`ff6740c2be58b79a926282c2b51690ee5810fdc3f61d5038730719c04a9a7280`.
The fixture matches the canonical form in Java and LLVM, not a runtime
authentication receipt.

## Fail-closed evaluator

- Reject wrong version/schema id/representation, malformed or ambiguous
  qualified ASCII aliases, malformed 64-character lowercase hex digests,
  absent predicates, inverted min/max and `validation_required=false`.
- Require descriptor/expected qualified type and fingerprint to match exactly.
  This catches accidental namespace or package ABI drift when expected values
  come from the *trusted compiler/loader*, not from a user argument.
- Reject wrong wire type, malformed signed-i64 number lexeme, noncanonical
  zeros/decimal/exponent encodings, integer overflow and input longer than
  the compiled registry's 20-byte number budget before parsing.
- Evaluate min then max in deterministic order using **signed i64 comparisons**,
  never generic floating-point `number_rule`.
- Return one bounded error with no value, payload, alias path or source text
  embedded. Successful `Result<int>` carries no validation provenance.

The v1 adapter does **not** consume or verify arbitrary JSON Schema,
TypeSpec, WIT, dynamic `where`, generic constraints, complex value trees,
compiled user functions, arbitrary `excluded` predicates or mutable aliases.
Qualified type identities are restricted to ASCII identifier segments in this
first runtime projection; unsupported compiler identifiers fail closed.

## Why the compiler fence remains

The Java draft `ConstraintValidationBridge` stages identity-bound host
evidence but exposes no verified guest checked-conversion path. The
source-language type checker still rejects constrained value usage with
`CONSTRAINT_VALIDATION_BRIDGE_REQUIRED`. **Do not remove that diagnostic**
because this Oreslang evaluator exists. A public descriptor or ordinary
`ValidationReport` can always be fabricated. A future compiler-minted
`Valid<T, SchemaId>` must require a privileged intrinsic, compiler-owned
nominal identity and generation, verified schema admission, transitive
immutable ownership, and no mutable aliases or post-validation TOCTOU.

## Acceptance gates

1. Execute the actual `.ores` tests in
   `tests/zorde_compiler_constraint_v1_test.ores` from `tests/main.ores`
   on an exact SHA-bound Java/GraalVM compiler/runtime and independently
   on native LLVM. Until both execute, the source is **NOT_VERIFIED**.
2. Produce a compiler-exported v1 catalog for all common-source/golden
   vectors; independently check canonical digest and exact type identity at
   build/load time and reject stale imported fingerprints.
3. Test hostile metadata: user/fake expected hash, swapped alias name,
   old schema generation, forged `ValidationReport`, raw `int` promotion,
   cross-actor/FFI rehydration, optimized expression substitution, and
   extension method call bypass. None may mint compiler proof.
4. Close ownership and freeze rules in the compiler, not a user-constructible
   runtime wrapper; reject unsupported targets explicitly. Keep Java/LLVM
   compile fences until these negative and positive executable gates pass.
5. Restore funded, approved source-local CI and protected main. Do not
   transfer private source across GitHub organizations without independently
   approved authorization and verified retention/security boundaries.

This adapter is intentionally distinct from the Zorde scalar runtime
`zorde.scalar/v1` descriptor: that descriptor covers scalar intervals and
Unicode lengths. The compiler `ores.type-constraint.v1` descriptor adds
qualified alias identity, fingerprint, validationRequired, explicit min/max
predicate presence and source-defined diagnostic IDs. They must not be
interchanged or treated as implicit authority.
