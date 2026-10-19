// Build-time only: fail-closed bridge from TJSV's derived Contract IR to the
// bounded Oreslang Zorde scalar registry. This does not authenticate producers,
// validate payloads, or prove either schema authority: the CI trust boundary
// must independently pin the actual TJSV action and source revision.
import { createHash } from 'node:crypto';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const HEX = /^[0-9a-f]{64}$/;
const SCALAR_ID = /^[A-Za-z][A-Za-z0-9_.:-]{0,127}$/;
const REPORT_KIND = 'ores.typespec-json-schema-validator.report/v1';
const IR_KIND = 'ores.typespec-json-schema-validator.contract-ir/v1';
const REQUIREMENTS = [
  'exactInputDigests', 'directDeclarationInventory',
  'generatedSchemaComparison', 'differentialInstanceValidation',
  'zeroUnexplainedFindings',
];
const ANNOTATIONS = new Set(['$id', '$schema', 'title', 'description']);
function assert(ok, message) { if (!ok) throw new Error('ZORDE_STATIC_REFUSED: ' + message); }
function object(x) { return x !== null && typeof x === 'object' && !Array.isArray(x); }
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function sha(x) { return typeof x === 'string' && HEX.test(x); }
function exactKeys(value, allowed, context) {
  assert(object(value), context + ' must be an object');
  for (const key of Object.keys(value)) {
    assert(allowed.has(key), context + ' contains unsupported keyword ' + key);
  }
}
function boundedInteger(n, lo, hi) {
  return Number.isSafeInteger(n) && n >= lo && n <= hi;
}

// Subset v1: fully bounded int32 scalar or code-point length constrained string.
// Every other JSON Schema keyword (including refs, coercion, format assertions,
// unions and defaults) is a hard error, not an ignored constraint.
export function descriptorFromSchema(schema) {
  assert(object(schema), 'scalar schema must be an object');
  assert(schema.type === 'integer' || schema.type === 'string', 'unsupported scalar type');
  const allowed = new Set(ANNOTATIONS);
  if (schema.type === 'integer') {
    for (const k of ['type', 'format', 'minimum', 'maximum']) allowed.add(k);
    exactKeys(schema, allowed, 'integer schema');
    assert(schema.format === undefined || schema.format === 'int32', 'unsupported integer format');
    assert(boundedInteger(schema.minimum, -2147483648, 2147483647), 'missing/unsafe integer minimum');
    assert(boundedInteger(schema.maximum, -2147483648, 2147483647), 'missing/unsafe integer maximum');
    assert(schema.minimum <= schema.maximum, 'empty integer interval');
    return {version:1, kind:2, property:1, minimum:schema.minimum,
      maximum:schema.maximum, excluded:[]};
  }
  for (const k of ['type', 'minLength', 'maxLength']) allowed.add(k);
  exactKeys(schema, allowed, 'string schema');
  assert(boundedInteger(schema.minLength, 0, 65536), 'missing/unsafe minLength');
  assert(boundedInteger(schema.maxLength, 0, 65536), 'missing/unsafe maxLength');
  assert(schema.minLength <= schema.maxLength, 'empty string length interval');
  return {version:1, kind:3, property:2, minimum:schema.minLength,
    maximum:schema.maxLength, excluded:[]};
}
function semanticDescriptor(schema) { return JSON.stringify(descriptorFromSchema(schema)); }
function emptyArray(x) { return Array.isArray(x) && x.length === 0; }
function sameDigestEvidence(ir, report, lane, reportLane) {
  assert(sha(ir.provenance?.[lane]?.digest), 'missing ' + lane + ' provenance digest');
  assert(ir.provenance[lane].digest === report.inputs?.[reportLane]?.digest,
    'input digest differs in ' + lane);
}

// This is an integrity-checked *build artifact*, never a public "admit" API.
// The expected digests must be independently provided by a trusted caller;
// a caller choosing hashes for its own forged IR has NOT authenticated it.
export function deriveStaticRegistry(ir, report) {
  assert(object(ir) && ir.schema === IR_KIND, 'wrong Contract IR schema');
  assert(ir.status === 'passed' && ir.admissible === true, 'Contract IR not admissible');
  assert(ir.role === 'downstream-derived-parity-artifact' && ir.editableAuthority === false,
    'Contract IR must be immutable derived evidence');
  assert(sha(ir.irId), 'invalid Contract IR identity');
  assert(ir.authorities?.typespec === 'independently-authored'
    && ir.authorities?.jsonSchema === 'independently-authored'
    && ir.authorities?.generatedJsonSchema === 'comparison-evidence-only'
    && ir.authorities?.precedence === 'none', 'peer authority boundary was changed');
  assert(object(report) && report.schema === REPORT_KIND
    && report.status === 'passed' && report.zeroUnexplainedFindings === true
    && sha(report.runId) && emptyArray(report.findings), 'unpassed TJSV report');
  assert(report.coverage?.directDeclarationInventory === true
    && report.coverage?.typespecGeneratedJsonSchemaComparison === true
    && report.coverage?.differentialInstanceValidation === true, 'TJSV coverage missing');
  assert(report.differential?.summary?.probesEvaluated > 0
    && report.differential?.summary?.divergences === 0
    && report.differential?.summary?.refusals === 0, 'differential evidence missing/refused');
  assert(ir.admission?.receipt?.schema === REPORT_KIND
    && ir.admission.receipt.runId === report.runId
    && ir.admission.receipt.status === 'passed'
    && ir.admission.receipt.zeroUnexplainedFindings === true
    && sha(ir.admission.receipt.digest), 'Contract IR/report binding mismatch');
  for (const key of REQUIREMENTS) assert(ir.admission?.requirements?.[key] === true,
    'missing IR requirement ' + key);
  assert(emptyArray(ir.excludedDeclarations) && emptyArray(ir.outOfScopeDeclarations),
    'unreviewed exclusions/out-of-scope declarations');
  assert(ir.admission?.scope?.complete === true
    && ir.admission.scope.excludedDeclarations === 0
    && ir.admission.scope.outOfScopeDeclarations === 0, 'incomplete IR scope');
  for (const [a,b] of [
    ['typespec','typespec'], ['generatedJsonSchema','generatedJsonSchema'],
    ['authoredJsonSchema','authoredJsonSchema']]) sameDigestEvidence(ir, report, a,b);
  assert(Array.isArray(ir.declarations) && ir.declarations.length >= 1
    && ir.declarations.length <= 64, 'registry has unsupported declaration count');
  assert(ir.admission.scope.admittedDeclarations === ir.declarations.length,
    'IR declaration count mismatch');
  assert(Array.isArray(report.declarationMap)
    && report.declarationMap.length === ir.declarations.length,
    'report declaration count mismatch');
  const mapped = new Map();
  for (const item of report.declarationMap) {
    assert(object(item) && typeof item.typespec === 'string'
      && !mapped.has(item.typespec), 'duplicate/invalid report declaration');
    mapped.set(item.typespec, item);
  }
  const seen = new Set();
  const entries = [];
  for (const decl of ir.declarations) {
    assert(object(decl) && decl.kind === 'scalar-like'
      && SCALAR_ID.test(decl.id) && !seen.has(decl.id),
      'unsupported/duplicate scalar declaration identity');
    seen.add(decl.id);
    assert(object(decl.names) && decl.names.typespec === decl.id,
      'nominal schema identity mismatched');
    const pair = mapped.get(decl.id);
    assert(pair && pair.generated === decl.names.generatedJsonSchema
      && pair.authored === decl.names.authoredJsonSchema,
      'report/IR declaration mapping mismatch');
    const generated = decl.lanes?.typespecGeneratedJsonSchema;
    const authored = decl.lanes?.authoredJsonSchema;
    assert(generated?.role === 'comparison-evidence-only'
      && authored?.role === 'independently-authored-authority',
      'declaration authority switched');
    assert(sha(decl.assertionDigest) && sha(generated.schemaDigest)
      && sha(authored.schemaDigest), 'missing declaration digests');
    const assertion = semanticDescriptor(decl.assertionSchema);
    assert(assertion === semanticDescriptor(generated.normalizedSchema)
      && assertion === semanticDescriptor(authored.normalizedSchema),
      'typed constraints drift between IR lanes');
    entries.push({schemaId:decl.id, descriptor:descriptorFromSchema(decl.assertionSchema)});
  }
  entries.sort((a,b) => a.schemaId < b.schemaId ? -1 : a.schemaId > b.schemaId ? 1 : 0);
  return {
    schema:'oreslang.zorde.static-scalar-registry-plan/v1',
    role:'non-authoritative-generated-build-artifact',
    irId:ir.irId, tjsvRunId:report.runId,
    sourceDigests:{
      typespec:ir.provenance.typespec.digest,
      authoredJsonSchema:ir.provenance.authoredJsonSchema.digest,
      generatedJsonSchema:ir.provenance.generatedJsonSchema.digest,
    },
    entries,
    runtimeImplementationVerified:false,
    actorSafeSharingVerified:false,
    releaseReady:false,
  };
}

function assertSafePlan(plan) {
  assert(object(plan) && plan.schema ===
    'oreslang.zorde.static-scalar-registry-plan/v1',
    'invalid generated plan');
  assert(plan.role === 'non-authoritative-generated-build-artifact'
    && plan.releaseReady === false
    && plan.runtimeImplementationVerified === false
    && plan.actorSafeSharingVerified === false,
    'plan cannot claim release/implementation verification');
  assert(sha(plan.irId) && sha(plan.tjsvRunId),
    'plan must bind a reviewed IR and TJSV receipt identity');
  assert(Array.isArray(plan.entries) && plan.entries.length >= 1
    && plan.entries.length <= 64, 'invalid plan entry count');
  const ids = new Set();
  for (const entry of plan.entries) {
    assert(object(entry) && SCALAR_ID.test(entry.schemaId)
      && !ids.has(entry.schemaId), 'unsafe/duplicate schema identity in plan');
    ids.add(entry.schemaId);
    const d = entry.descriptor;
    assert(object(d) && d.version === 1 && Array.isArray(d.excluded)
      && d.excluded.length === 0, 'unsupported descriptor/exclusions in plan');
    if (d.kind === 2) {
      assert(d.property === 1
        && boundedInteger(d.minimum,-2147483648,2147483647)
        && boundedInteger(d.maximum,-2147483648,2147483647),
        'unsafe integer plan descriptor');
    } else if (d.kind === 3) {
      assert(d.property === 2
        && boundedInteger(d.minimum,0,65536)
        && boundedInteger(d.maximum,0,65536),
        'unsafe string plan descriptor');
    } else assert(false, 'unsupported plan kind');
    assert(d.minimum <= d.maximum, 'empty plan interval');
    assert(Object.keys(d).sort().join(',') ===
      ['excluded','kind','maximum','minimum','property','version'].join(','),
      'unexpected descriptor fields');
  }
}

export function emitOreslangSpecs(plan) {
  assertSafePlan(plan);
  const lines = [
    '// GENERATED from digest-bound TJSV Contract IR; never an authority.',
    '// Review the accepted source and toolchain before using this at runtime.',
    "import class Utf8Text from '../../src/core/value';",
    "import class ZordeScalarDescriptor from '../../src/zorde/refinement_descriptor';",
    "import class ZordeScalarRegistrySpec from '../../src/zorde/compiled_registry';",
    'pub fnc zorde_contract_registry_specs(): List<ZordeScalarRegistrySpec> {',
    '  val List<ZordeScalarRegistrySpec> specs = new List<ZordeScalarRegistrySpec>();',
  ];
  for (const {schemaId,descriptor:d} of plan.entries) {
    const bytes = [...Buffer.from(schemaId,'ascii')].join(',');
    lines.push('  specs.add(new ZordeScalarRegistrySpec(');
    lines.push('    new Utf8Text(new List<int>(' + bytes + ')),');
    lines.push('    new ZordeScalarDescriptor(1,' + d.kind + ',' + d.property + ','
      + d.minimum + ',' + d.maximum + ',new List<int>())');
    lines.push('  ));');
  }
  lines.push('  return specs;');
  lines.push('}');
  return lines.join('\n') + '\n';
}
function file(path, expected) {
  assert(sha(expected), 'explicit expected SHA-256 is required');
  const bytes = readFileSync(path);
  assert(bytes.length > 0 && bytes.length <= 2097152, 'input size outside budget');
  assert(digest(bytes) === expected, 'input byte digest differs from trusted expectation');
  return JSON.parse(bytes.toString('utf8'));
}
export function main(args) {
  const flags = new Map();
  for (const arg of args) {
    const p = arg.indexOf('=');
    assert(p > 2 && arg.startsWith('--'), 'expected --name=value');
    const key = arg.slice(2,p);
    assert(['ir','report','ir-sha256','report-sha256','out-dir'].includes(key)
      && !flags.has(key), 'unknown/duplicate argument ' + key);
    flags.set(key,arg.slice(p+1));
  }
  assert(flags.size === 5 && [...flags.values()].every(Boolean),
    'all five CLI arguments are mandatory');
  const ir = file(flags.get('ir'), flags.get('ir-sha256'));
  const report = file(flags.get('report'), flags.get('report-sha256'));
  const plan = deriveStaticRegistry(ir,report);
  const dir = resolve(flags.get('out-dir'));
  // Imports in emitted Oreslang file assume artifacts/zorde-generated/; refuse
  // ambiguous location rather than quietly generating broken guest imports.
  assert(relative(resolve('artifacts/zorde-generated'),dir) === '',
    'output must be the repository artifacts/zorde-generated directory');
  mkdirSync(dir,{recursive:true});
  writeFileSync(resolve(dir,'registry-plan.json'),JSON.stringify(plan,null,2)+'\n',{flag:'wx',mode:0o600});
  writeFileSync(resolve(dir,'registry-specs.ores'),emitOreslangSpecs(plan),{flag:'wx',mode:0o600});
  process.stdout.write('ZORDE_STATIC_PLAN_CREATED (not runtime/proof evidence)\n');
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); } catch (e) {
    process.stderr.write(String(e?.message || e)+'\n');
    process.exitCode=2;
  }
}
