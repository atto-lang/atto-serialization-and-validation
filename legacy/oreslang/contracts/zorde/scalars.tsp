// Independent TypeSpec authority for the *restricted*, statically compiled
// Zorde scalar contract subset. Never generated from scalars.schema.json.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;
namespace Zorde.Validation;

@id("Port")
@minValue(1)
@maxValue(65535)
scalar Port extends int32;

@id("ShortLabel")
@minLength(1)
@maxLength(64)
scalar ShortLabel extends string;
