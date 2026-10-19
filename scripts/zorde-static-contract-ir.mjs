// Build-time only: fail-closed bridge from TJSV's derived Contract IR to the
// bounded Oreslang Zorde scalar registry. This does not authenticate producers,
// validate payloads, or prove either schema authority: the CI trust boundary
// must independently pin the actual TJSV action and source revision.
import { createHash } from 'node:crypto';
import { openSync, closeSync, readSync, fstatSync, lstatSync, existsSync, mkdirSync,
  writeFileSync, constants as fsConstants } from 'node:fs';
import { relative, resolve } from 'node:path';
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
// Match the pinned upstream TJSV Contract IR canonical JSON SHA-256 scheme.
function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (!object(value)) return value;
  return Object.fromEntries(Object.keys(value).sort().map(
    key=>[key,canonicalJson(value[key])]));
}
export function canonicalDigest(value) {
  return digest(Buffer.from(JSON.stringify(canonicalJson(value)),'utf8'));
}
export function semanticReportProjection(report) {
  const projected=structuredClone(report);
  assert(object(projected.configuration) && object(projected.toolchain)
    && object(projected.inputs), 'TJSV report identity inputs are missing');
  const c=projected.configuration;
  c.emitterOptions=c.emitterOptions ?
    Object.fromEntries(Object.entries(c.emitterOptions)
      .filter(([key])=>key!=='emitter-output-dir')) : null;
  c.executionMode=null;
  c.differential={...c.differential,instanceCorpus:null};
  projected.toolchain.typespecCompiler={
    version:projected.toolchain.typespecCompiler?.version??null};
  for (const lane of ['typespec','authoredJsonSchema','generatedJsonSchema']) {
    assert(object(projected.inputs[lane]),'missing source lane: '+lane);
    projected.inputs[lane]={...projected.inputs[lane],input:null};
  }
  if(Object.hasOwn(projected.inputs,'mapping'))projected.inputs.mapping=null;
  return projected;
}
function sha(x) { return typeof x === 'string' && HEX.test(x); }
function exactKeys(value, allowed, context) {
  assert(object(value), context + ' must be an object');
  for (const key of Object.keys(value)) {
    assert(allowed.has(key), context + ' contains unsupported keyword');
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
  assert(Object.hasOwn(schema,'type')
    && (schema.type === 'integer' || schema.type === 'string'), 'unsupported scalar type');
  if(Object.hasOwn(schema,'$schema')) assert(
    schema.$schema==='https://json-schema.org/draft/2020-12/schema',
    'unsupported JSON Schema dialect');
  const allowed = new Set(ANNOTATIONS);
  if (schema.type === 'integer') {
    for (const k of ['type', 'format', 'minimum', 'maximum']) allowed.add(k);
    exactKeys(schema, allowed, 'integer schema');
    assert(schema.format === undefined || schema.format === 'int32', 'unsupported integer format');
    assert(Object.hasOwn(schema,'minimum')
      && boundedInteger(schema.minimum, -2147483648, 2147483647), 'missing/unsafe integer minimum');
    assert(Object.hasOwn(schema,'maximum')
      && boundedInteger(schema.maximum, -2147483648, 2147483647), 'missing/unsafe integer maximum');
    assert(schema.minimum <= schema.maximum, 'empty integer interval');
    return {version:1, kind:2, property:1, minimum:schema.minimum,
      maximum:schema.maximum, excluded:[]};
  }
  for (const k of ['type', 'minLength', 'maxLength']) allowed.add(k);
  exactKeys(schema, allowed, 'string schema');
  assert(Object.hasOwn(schema,'minLength')
    && boundedInteger(schema.minLength, 0, 65536), 'missing/unsafe minLength');
  assert(Object.hasOwn(schema,'maxLength')
    && boundedInteger(schema.maxLength, 0, 65536), 'missing/unsafe maxLength');
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
  const {irId:_,...irBody}=ir;
  assert(canonicalDigest(irBody)===ir.irId, 'Contract IR self digest mismatch');
  assert(ir.authorities?.typespec === 'independently-authored'
    && ir.authorities?.jsonSchema === 'independently-authored'
    && ir.authorities?.generatedJsonSchema === 'comparison-evidence-only'
    && ir.authorities?.precedence === 'none', 'peer authority boundary was changed');
  assert(object(report) && report.schema === REPORT_KIND
    && report.status === 'passed' && report.zeroUnexplainedFindings === true
    && sha(report.runId) && emptyArray(report.findings), 'unpassed TJSV report');
  assert(object(report.counts) && report.counts.findings===0
    && report.counts.structuralFindings===0
    && report.counts.differentialFindings===0
    && report.counts.emittedFindings===0
    && report.counts.findingsTruncated===false,
    'TJSV report contains missing/truncated or nonzero findings');
  assert(report.configuration?.mode==='check'
    && report.coverage?.sourceMutationCheck===true
    && report.coverage?.directDeclarationInventory === true
    && report.coverage?.typespecGeneratedJsonSchemaComparison === true
    && report.coverage?.differentialInstanceValidation === true,
    'TJSV full source-backed check coverage missing');
  assert(report.differential?.summary?.probesEvaluated > 0
    && report.differential?.summary?.divergences === 0
    && report.differential?.summary?.refusals === 0, 'differential evidence missing/refused');
  // Recompute the actual TJSV runId, not merely its SHA-256 syntax. Binding
  // source digests into a rewritten receipt is necessary but not proof of a
  // trusted producer or current checked-out source; those are CI obligations.
  const normalizedReport=semanticReportProjection(report);
  const expectedRunId=canonicalDigest({
    schema:REPORT_KIND,status:report.status,
    configuration:normalizedReport.configuration,
    inputDigests:{
      typespec:report.inputs.typespec.digest,
      authored:report.inputs.authoredJsonSchema.digest,
      generated:report.inputs.generatedJsonSchema.digest,
    },
    toolchain:normalizedReport.toolchain,
    findings:report.findings.map(f=>f.fingerprint),
  });
  assert(expectedRunId===report.runId, 'TJSV runId content digest mismatch');
  assert(ir.admission?.receipt?.schema === REPORT_KIND
    && ir.admission.receipt.runId === report.runId
    && ir.admission.receipt.status === 'passed'
    && ir.admission.receipt.zeroUnexplainedFindings === true
    && sha(ir.admission.receipt.digest)
    && canonicalDigest(normalizedReport)===ir.admission.receipt.digest,
    'Contract IR/report binding mismatch');
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
    assert(pair && ['scalar','alias','scalar-like'].includes(pair.kind),
      'mapped declaration is not a scalar kind');
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
    assert(canonicalDigest(decl.assertionSchema)===decl.assertionDigest,
      'assertion schema digest no longer matches its body');
    assert(canonicalDigest(generated.normalizedSchema)===generated.schemaDigest,
      'generated schema lane digest no longer matches');
    assert(canonicalDigest(authored.normalizedSchema)===authored.schemaDigest,
      'authored schema lane digest no longer matches');
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
  assert(object(plan.sourceDigests)
    && Object.keys(plan.sourceDigests).sort().join(',') ===
      ['authoredJsonSchema','generatedJsonSchema','typespec'].join(',')
    && sha(plan.sourceDigests.typespec)
    && sha(plan.sourceDigests.authoredJsonSchema)
    && sha(plan.sourceDigests.generatedJsonSchema),
    'plan must bind exactly three valid peer-authority SHA-256 digests');
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
// Pilot ONLY: a real Oreslang guest program that imports the emitted registry
// specs and exercises positive/negative bounds using the existing native
// registry. The guest must actually compile/execute before anyone may claim
// native validation, Java/LLVM parity or schema-to-runtime correspondence.
// Future generic guest generators must derive witnesses for every refinement.
export function emitZordePilotGuestSmoke(plan) {
  assertSafePlan(plan);
  const port=plan.entries.find(x=>x.schemaId==='Zorde.Validation.Port');
  const label=plan.entries.find(x=>x.schemaId==='Zorde.Validation.ShortLabel');
  assert(plan.entries.length===2 && port && label,
    'pilot guest requires exactly the reviewed Port and ShortLabel declarations');
  assert(port.descriptor.kind===2 && port.descriptor.property===1
    && port.descriptor.minimum===1 && port.descriptor.maximum===65535,
    'pilot Port descriptor changed: re-review the actual guest assertions');
  assert(label.descriptor.kind===3 && label.descriptor.property===2
    && label.descriptor.minimum===1 && label.descriptor.maximum===64,
    'pilot ShortLabel descriptor changed: re-review the actual guest assertions');
  const bytes=x=>[...Buffer.from(x,'ascii')].join(',');
  const lines=[
    '// GENERATED pilot guest: not a proof until the approved Oreslang compiler',
    '// executes this file at the exact IR/source-bound revision.',
    "import fnc require_true from '../../tests/assert';",
    "import fnc zorde_contract_registry_specs from './registry-specs';",
    "import class ZordeScalarRegistry, ZordeScalarRegistrySpec from '../../src/zorde/compiled_registry';",
    "import fnc zorde_compile_scalar_registry, zorde_registry_parse_scalar from '../../src/zorde/compiled_registry';",
    "import class Utf8Text from '../../src/core/value';",
    "import fnc value_int, value_string from '../../src/core/value';",
    "import class ValidationReport from '../../src/validation/validation';",
    'pub fnc main(): void {',
    '  val List<ZordeScalarRegistrySpec> specs = zorde_contract_registry_specs();',
    '  val Result<ZordeScalarRegistry,ValidationReport> result =',
    '    zorde_compile_scalar_registry(rt borrow specs);',
    '  require_true(!result.is_err(), "generated registry specs must compile");',
    '  val ZordeScalarRegistry registry = result.unwrap();',
    '  val Utf8Text port = new Utf8Text(new List<int>('+bytes(port.schemaId)+'));',
    '  val Utf8Text label = new Utf8Text(new List<int>('+bytes(label.schemaId)+'));',
    '  require_true(!zorde_registry_parse_scalar(rt borrow registry, port,',
    '    value_int(1)).is_err(), "Port lower boundary accepts");',
    '  require_true(!zorde_registry_parse_scalar(rt borrow registry, port,',
    '    value_int(65535)).is_err(), "Port upper boundary accepts");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, port,',
    '    value_int(0)).is_err(), "Port below minimum rejected");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, port,',
    '    value_int(65536)).is_err(), "Port above maximum rejected");',
    '  require_true(!zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>(97)))).is_err(),',
    '    "ShortLabel one character accepted");',
    '  require_true(!zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>('+Array(64).fill(97).join(',')+')))).is_err(),',
    '    "ShortLabel 64 characters accepted");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>()))).is_err(),',
    '    "ShortLabel empty rejected");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>('+Array(65).fill(97).join(',')+')))).is_err(),',
    '    "ShortLabel 65 characters rejected");',
    '  val Result<int,ValidationReport> composed =',
    '    zorde_registry_parse_scalar(rt borrow registry, label,',
    '      value_string(new Utf8Text(new List<int>(195,169,240,159,152,128))));',
    '  require_true(!composed.is_err(), "multibyte Unicode scalars accepted");',
    '  require_true(composed.unwrap() == 2,',
    '    "UTF-8 byte length cannot replace Unicode scalar count");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>(237,160,128)))).is_err(),',
    '    "UTF-8 surrogate encoding rejected");',
    '  require_true(zorde_registry_parse_scalar(rt borrow registry, label,',
    '    value_string(new Utf8Text(new List<int>(192,175)))).is_err(),',
    '    "malformed UTF-8 rejected");',
    '  stdio.stdout.println("zorde generated registry guest smoke passed");',
    '  return;',
    '}',
  ];
  return lines.join('\n')+'\n';
}

// Duplicate JSON keys are ambiguous across tooling; even "\u0072ole"
// duplicates "role". Detect decoded object-key collisions before JSON.parse.
// The built-in parser still performs full JSON syntax/number validation.
export function rejectDuplicateJsonKeys(source) {
  assert(typeof source === 'string' && source.length <= 2097152,
    'JSON source size outside bounds');
  let i=0, nodes=0;
  const whitespace=()=>{while(i<source.length && ' \t\r\n'.includes(source[i]))i++;};
  function scanString() {
    assert(source[i] === '"', 'expected JSON string');
    const start=i++;
    let closed=false;
    while(i<source.length) {
      const ch=source[i++];
      if(ch==='\\') { i++; continue; }
      if(ch==='"') { closed=true; break; }
    }
    assert(closed, 'unterminated JSON string');
    return JSON.parse(source.slice(start,i));
  }
  function scanValue(depth) {
    assert(depth<=64 && ++nodes<=100000, 'JSON structure exceeds work budget');
    whitespace();
    const next=source[i];
    if(next==='{') {
      i++;whitespace();
      const keys=new Set();
      if(source[i]==='}') {i++;return;}
      while(i<source.length) {
        const key=scanString();
        assert(!keys.has(key), 'duplicate JSON object key');
        keys.add(key);
        whitespace();
        assert(source[i]===':', 'JSON object colon missing');
        i++;
        scanValue(depth+1);whitespace();
        if(source[i]==='}') {i++;return;}
        assert(source[i]===',','JSON object separator missing');
        i++;whitespace();
      }
      assert(false,'unterminated JSON object');
    }
    if(next==='[') {
      i++;whitespace();
      if(source[i]===']') {i++;return;}
      while(i<source.length) {
        scanValue(depth+1);whitespace();
        if(source[i]===']') {i++;return;}
        assert(source[i]===',','JSON array separator missing');
        i++;
      }
      assert(false,'unterminated JSON array');
    }
    if(next==='"') {scanString();return;}
    const start=i;
    while(i<source.length && !',]} \t\r\n'.includes(source[i]))i++;
    assert(start!==i,'missing JSON primitive');
  }
  whitespace();scanValue(0);whitespace();
  assert(i===source.length,'unexpected trailing JSON tokens');
  JSON.parse(source); // syntax check independent from the collision scan
}

// Open the reviewed input without following a symbolic link. Check type and
// size *before* allocating/reading, then allow at most max+1 bytes even if a
// concurrent writer grows the file after fstat. Hash the actual read bytes.
export function readEvidenceFile(path, expected) {
  assert(sha(expected), 'explicit expected SHA-256 is required');
  // O_NOFOLLOW is unsupported on some Node/OS combinations. Silent
  // undefined-to-zero bitwise coercion would follow an attacker-controlled
  // symlink, so stop before open() rather than weakening the boundary.
  assert(Number.isInteger(fsConstants.O_NOFOLLOW) && fsConstants.O_NOFOLLOW !== 0,
    'platform cannot guarantee O_NOFOLLOW evidence reads');
  assert(Number.isInteger(fsConstants.O_NONBLOCK),
    'platform cannot guarantee bounded nonblocking evidence opens');
  let fd=-1;
  try {
    fd=openSync(path,fsConstants.O_RDONLY|fsConstants.O_NOFOLLOW|fsConstants.O_NONBLOCK);
    const st=fstatSync(fd);
    assert(st.isFile() && st.nlink===1, 'source must be a unique regular file');
    assert(st.size>0 && st.size<=2097152, 'input size outside budget');
    const bytes=Buffer.alloc(st.size+1);
    let count=0;
    while(count<bytes.length){
      const n=readSync(fd,bytes,count,bytes.length-count,null);
      if(n===0)break;
      count+=n;
    }
    assert(count===st.size, 'source changed or expanded while reading');
    const current=fstatSync(fd);
    assert(current.size===st.size && current.ino===st.ino && current.dev===st.dev,
      'source file changed while reading');
    const input=bytes.subarray(0,count);
    assert(digest(input)===expected, 'input byte digest differs from trusted expectation');
    const source=new TextDecoder('utf-8',{fatal:true}).decode(input);
    rejectDuplicateJsonKeys(source);
    return JSON.parse(source);
  } finally {
    if(fd!==-1)closeSync(fd);
  }
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
  const ir = readEvidenceFile(flags.get('ir'), flags.get('ir-sha256'));
  const report = readEvidenceFile(flags.get('report'), flags.get('report-sha256'));
  const plan = deriveStaticRegistry(ir,report);
  const dir = resolve(flags.get('out-dir'));
  // Imports in emitted Oreslang file assume artifacts/zorde-generated/; refuse
  // ambiguous location rather than quietly generating broken guest imports.
  assert(relative(resolve('artifacts/zorde-generated'),dir) === '',
    'output must be the repository artifacts/zorde-generated directory');
  // Prepare every output before creating a directory. Reject an existing
  // target instead of trusting old/stale receipts or following a symlink.
  const guest=emitOreslangSpecs(plan);
  const guestSmoke=emitZordePilotGuestSmoke(plan);
  const planBytes=JSON.stringify(plan,null,2)+'\n';
  const artifacts=resolve('artifacts');
  if(!existsSync(artifacts))mkdirSync(artifacts,{mode:0o700});
  const dirStat=lstatSync(artifacts);
  assert(dirStat.isDirectory() && !dirStat.isSymbolicLink(),
    'artifacts parent must be a real directory');
  assert(!existsSync(dir),'output directory already exists');
  mkdirSync(dir,{mode:0o700});
  const createdDir=lstatSync(dir);
  assert(createdDir.isDirectory() && !createdDir.isSymbolicLink(),
    'output directory is not safe');
  // A partial failure leaves no completed marker. Consumers must require
  // this final marker as well as separately reviewed source provenance.
  writeFileSync(resolve(dir,'registry-plan.json'),planBytes,{flag:'wx',mode:0o600});
  writeFileSync(resolve(dir,'registry-specs.ores'),guest,{flag:'wx',mode:0o600});
  writeFileSync(resolve(dir,'registry-smoke.ores'),guestSmoke,{flag:'wx',mode:0o600});
  writeFileSync(resolve(dir,'build-complete.json'),JSON.stringify({
    schema:'oreslang.zorde.static-registry-build-complete/v1',
    irId:plan.irId, tjsvRunId:plan.tjsvRunId,
    planSha256:digest(Buffer.from(planBytes,'utf8')),
    guestSha256:digest(Buffer.from(guest,'utf8')),
    smokeSha256:digest(Buffer.from(guestSmoke,'utf8')),
    guestSmokeCompiled:false, guestSmokeExecuted:false,
    nativeGuestExecuted:false, releaseReady:false,
  },null,2)+'\n',{flag:'wx',mode:0o600});
  process.stdout.write('ZORDE_STATIC_PLAN_CREATED (not runtime/proof evidence)\n');
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); } catch (e) {
    process.stderr.write(String(e?.message || e)+'\n');
    process.exitCode=2;
  }
}
