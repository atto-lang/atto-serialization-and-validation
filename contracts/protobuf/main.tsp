// Independent TypeSpec authority for a *diagnostic* wire-field envelope.
// The real protobuf bytes remain protobuf, not JSON. No code generation or
// schema approval follows from this file without independent parity checks.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace Oreslang.Protobuf;

@id("WireField")
model WireField {
  // The 19000..19999 declaration restriction is enforced by the trusted
  // typed descriptor/Zorde adapter, not by the unknown-field wire parser.
  @minValue(1)
  @maxValue(536870911)
  fieldNumber: int32;
  wireType: 0 | 1 | 2 | 5;
  // Raw varint/fixed-width/length-delimited content, excluding tag/length.
  payload: uint8[];
}

@id("WireMessage")
model WireMessage {
  fields: WireField[];
}
