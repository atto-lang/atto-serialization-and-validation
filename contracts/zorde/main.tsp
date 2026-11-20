// Independently authored authority A. The generated JSON Schema is only evidence.
// Do not synthesize or rewrite contracts/zorde/authored.schema.json from this file.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace Zorde.Validation;

@id("ErrorCode")
enum ErrorCode {
  missingRequired: "missing-required",
  wrongType: "wrong-type",
  unknownField: "unknown-field",
  outOfRange: "out-of-range",
}

@id("ComparisonStatus")
enum ComparisonStatus {
  equivalent: "equivalent",
  different: "different",
  refused: "refused",
}

@id("ValidationIssue")
model ValidationIssue {
  code: ErrorCode;
  path: string;
}

@id("ValidationReport")
model ValidationReport {
  schemaVersion: 1;
  valid: boolean;
  issues: ValidationIssue[];
}

@id("SchemaComparison")
model SchemaComparison {
  schemaVersion: 1;
  status: ComparisonStatus;
  authoredDigest: string;
  generatedDigest: string;
  mismatchPaths: string[];
}
