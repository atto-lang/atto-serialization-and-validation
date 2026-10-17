# Zorde

**Zorde** is Oreslang's schema, validation, and refinement-runtime library.

It is the ergonomic public layer over the existing native reflection-free
validation engine in this repository. Zorde does not create a second validation
implementation: compiler refinements, generated codecs, standalone application
validation, and decode-time validation normalize to one constraint model.

## Core contract

Oreslang refinements have two enforcement phases:

1. **Compile time** — the compiler proves constraints when the value is
   statically known or the source refinement entails the destination.
2. **Runtime boundary** — Zorde validates values that originate outside the
   proof domain: network payloads, files, databases, environment/config,
   command-line input, FFI, dynamic maps, deserialization, and other dynamic
   or untrusted values.

A successful runtime validation establishes validated typestate. The same
immutable value should not be repeatedly revalidated merely because it crosses
ordinary internal function calls.

    type NonEmpty = string with length gt 0;
    type Username = string with length ge 3 and length le 32;

    const NonEmpty a = "ores";   // compile-time proof
    const NonEmpty b = "";       // compile-time error

    val ZordeValueSchema schema = zorde_nonempty_string();
    val Result<ValidValue, ValidationReport> checked =
      zorde_parse(raw_value, schema);

The compiler integration should generate the Zorde schema from the normalized
refinement descriptor rather than requiring users to restate constraints.

## Three declaration syntaxes, one runtime schema

All accepted refinement syntax families lower into the same normalized
constraint descriptor:

    @Size(min=1, max=32)
    type A = string;

    type B = string with length ge 1 and length le 32;

    define newtype C(string)
      with length ge 1 and length le 32;

A and B have equivalent refinement predicates. C adds nominal identity. For
dynamic construction, compiler-generated C.try_from(...) must call the same
generated/specialized predicate used by Zorde.

General where predicates remain pure and deterministic. A where predicate may
participate in runtime validation only when the compiler can emit a bounded,
effect-checked validator. Unsupported predicates fail closed; Zorde must never
silently erase a constraint.

## Where refinements belong

Refined aliases and nominal newtypes are the reusable form, but the type system
should eventually permit refinement-bearing types anywhere an ordinary value
type can appear:

- class and struct fields;
- function parameters and return types;
- locals and constants;
- tuple elements;
- list/array element types;
- map key/value types;
- actor DATA message fields;
- RPC/HTTP request and response DTO fields;
- decoded configuration and environment values;
- database/ORM row and column projections;
- FFI/import/export boundary values;
- serialization/deserialization schemas;
- persistent actor state and hot-reload state migration contracts.

Examples:

    type Port = int with value ge 1 and value le 65535;
    type Percentage = int with value ge 0 and value le 100;
    type NonEmpty = string with length gt 0;
    type Sha256 = bytes with length eq 32;
    type NonEmptyList<T> = List<T> with length gt 0;

    define struct User as
      name: NonEmpty;
      age: int with value ge 0 and value le 150;
    end

Inline refined type expressions are a useful follow-up grammar feature:

    fnc greet(string with length gt 0 name): void;
    fnc choose(): int with value ge 0 and value lt 10;

Until inline type-expression grammar is implemented, aliases/newtypes provide
the same invariant without adding special parameter/field syntax.

## Flow-sensitive refinement

The compiler should also learn refinements from ordinary control flow when the
predicate exactly matches a supported built-in constraint:

    fnc use(string value): void {
      if value.length gt 0 then
        // value can be treated as string with length gt 0 here.
      fi
    }

This is a compiler proof feature, not a runtime Zorde check. Arbitrary where
predicates do not imply a general theorem prover.

## Runtime trust boundaries

The compiler should not insert surprise validation on arbitrary assignments.
Runtime checks belong at explicit trust/construction boundaries:

- Newtype.try_from(raw);
- Zorde parse / safe-parse equivalents;
- JSON/MessagePack/CBOR/Protobuf typed decode;
- HTTP/RPC ingress;
- actor DATA admission;
- database/config/environment decoding;
- FFI/imported host values;
- hot-reload state migration.

Network and actor serialization remain distinct capability domains. Supervisor
CONTROL messages are sealed runtime protocol and are not made user-validatable
merely by Zorde.

## Schema generation

The target API is reflection-free. A future compiler intrinsic such as:

    val schema = zorde.schema<Username>();

must lower at compile time to a static generated descriptor/validator. It must
not discover fields or predicates through runtime reflection.

For a known object type, generated decode should fuse construction and
validation so invalid data is rejected before a complete invalid object is
materialized. Standalone Zorde validation remains useful for
application-created values and dynamic Value trees.

## Current native facade

src/zorde/zorde.ores currently exposes an ergonomic facade over the existing
validator:

- zorde_string(min, max)
- zorde_nonempty_string()
- zorde_exact_string(length)
- zorde_ascii_string(min, max)
- zorde_email()
- zorde_uuid()
- zorde_url()
- zorde_number(...)
- zorde_array(...)
- zorde_field(...)
- zorde_object(...)
- zorde_parse(...)
- zorde_parse_object(...)

The existing validation engine remains the implementation underneath. This
initial facade does not claim compiler-generated zorde.schema<T>(), arbitrary
where execution, regex/pattern validation, cross-field predicates, or full
typed-object construction.

## Refinement-to-Zorde normalization

Built-in compiler predicates should normalize mechanically:

| Compiler refinement | Zorde runtime rule |
| --- | --- |
| string with length gt 0 | string min length 1 |
| string with length eq N | string min/max length N |
| string with length ge A and length le B | string min/max A/B |
| int with value ge A and value le B | numeric inclusive range |
| int with value gt A and value lt B | numeric exclusive range |
| List<T> with length ge A | array min items A |
| Map<K,V> with size le B | object/map max entries B (future native rule) |
| bytes with length eq N | byte exact length N |

ne/excluded-value predicates, map-size predicates, and arbitrary where
predicates require explicit runtime representations before the compiler may
claim complete Zorde parity. Unsupported descriptors are errors, not ignored
metadata.

The current generic numeric ValueRule path materializes wire numbers through a
float for range validation. It therefore must **not** be used to claim exact
64-bit integer-refinement parity outside the exactly representable range.
Compiler-generated int refinements need an integer-exact Zorde rule/validator
before that boundary is marked supported.

## ABI and hot reload

Exported refinement predicates and Zorde schema fingerprints are part of the
type contract. Tightening a public constraint can reject previously valid
values and therefore participates in ABI/hot-reload compatibility checks.

A schema fingerprint should include at least:

- base/nominal type identity;
- normalized constraint predicates;
- Unicode length semantics;
- object required/optional/forbidden fields;
- unknown-field policy;
- nested schema identities;
- where predicate identity/version when present.

The compiler, Zorde, serializers, actor admission, and hot-reload verifier
should consume the same fingerprint rather than maintaining parallel notions
of validity.
