# Proposed Oreslang surface

This is a design sketch, not yet a parser conformance test.

```ores
@Serializable
@UnknownFields(reject)
define class User as
  @Required
  @MinLength(3)
  @MaxLength(64)
  val string username;

  @Required
  @Range(0, 150)
  val int age;

  @WireName("display_name")
  @MaxLength(120)
  val string displayName;
end

val User user = json.decode<User>(bytes);
val Map<string, Value> unknown = json.decode_map(bytes);

val decoder = json.decoder<User>();
for chunk of chunks do
  match decoder.feed(chunk) {
    NeedMore -> { }
    ValueReady(value) -> {
      consume(value);
    }
    DecodeFailed(error) -> {
      return Err(error);
    }
  }
done
```

The compiler should lower the annotations into static codec/validation metadata and generate the typed decode path. No runtime reflection is required.

For raw high-throughput routing or inspection:

```ores
val ValueView view = json.parse_view(bytes);
val string_view kind = view.get_string("kind");
```

A view borrows immutable input. Materializing or retaining data beyond the input lifetime requires an explicit copy/owned conversion under Oreslang ownership rules.
