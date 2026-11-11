#!/usr/bin/env node
/**
 * Static schema/WIT projection guard; deliberately NOT a WIT parser or ABI proof.
 * The independently authored JSON Schema stays authoritative for JSON values.
 * A separate pinned wasm-tools run validates the actual WIT grammar.
 */
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {rejectDuplicateJsonKeys} from './zorde-static-contract-ir.mjs';

function refuse(message) { throw new Error('ZORDE_WIT_STATIC_CROSSWALK_DENIED: '+message); }
function matchesExactly(left,right) {
  return Array.isArray(left) && left.length===right.length &&
    left.every((v,i)=>v===right[i]);
}
function exactSchemaKeys(value, allowed, context) {
  if(!value || typeof value!=='object' || Array.isArray(value) ||
     Object.keys(value).length!==allowed.length ||
     allowed.some(k=>!Object.hasOwn(value,k))) {
    refuse('unexpected schema keyword or shape for '+context);
  }
}
function exactSchemaMetadata(value, name) {
  if(value?.$schema!=='https://json-schema.org/draft/2020-12/schema' ||
     value?.$id!==name) refuse('schema identifier or dialect changed for '+name);
}
function bodyOf(text,kind,name) {
  // The three selected enum/record blocks have no nested WIT blocks.
  // This is only a conservative projection check, not a language parser.
  const rx=new RegExp('\\b'+kind+'\\s+'+name+'\\s*\\{([^{}]*)\\}','g');
  const hits=[...text.matchAll(rx)];
  if(hits.length!==1) refuse('missing or duplicate '+kind+' '+name);
  return hits[0][1];
}
function tokens(body) {
  const result=body.split(',').map(x=>x.trim()).filter(Boolean);
  if(result.length===0 || result.some(x=>x.includes(';')||x.includes('{')||x.includes('}')))
    refuse('unrecognized field declaration');
  return result;
}
function exactSchemaFields(def,propertyNames,requiredNames) {
  if(def?.type!=='object' || def.unevaluatedProperties!==false ||
     !matchesExactly(Object.keys(def.properties||{}),propertyNames) ||
     !matchesExactly(def.required,requiredNames)) refuse('authored schema record shape changed');
}
// JSON.parse alone discards duplicate object members (including escaped
// equivalents), allowing the last key to override independently authored
// constraints. Admit raw bytes before normal JSON materialization.
export function parseAuthoredZordeSchema(rawSource) {
  if(typeof rawSource!=='string' ||
     Buffer.byteLength(rawSource,'utf8')>2097152)
    refuse('authored schema raw input exceeds bounded UTF-8 budget');
  try {
    rejectDuplicateJsonKeys(rawSource);
    return JSON.parse(rawSource);
  } catch {
    refuse('authored schema contains duplicate, malformed or unbounded JSON');
  }
}

export function auditZordeWitProjection(rawWit, jsonSchema) {
  if(typeof rawWit!=='string'||!jsonSchema||typeof jsonSchema!=='object')
    refuse('invalid source documents');
  // Reject block comments and WIT annotations until separately modeled.
  // Strip only simple line comments. wasm-tools validates grammar independently.
  if(rawWit.includes('/*')||rawWit.includes('*/')||rawWit.includes('@unstable'))
    refuse('unsupported WIT syntax in static crosswalk');
  const wit=rawWit.replace(/\/\/[^\n]*/g,'');
  // A partial regex match could otherwise approve a valid-looking interface
  // followed by extra operations, exports or a substituted package. Refuse
  // unreviewed WIT surface until the projection mapping is expanded deliberately.
  // This deliberately restrictive grammar is NOT a substitute for wasm-tools.
  if(rawWit.length>16384 ||
     !/^\s*package\s+oreslang:zorde-validation@0\.1\.0\s*;\s*interface\s+validation\s*\{\s*enum\s+error-code\s*\{[^{}]*\}\s*record\s+validation-issue\s*\{[^{}]*\}\s*record\s+validation-report\s*\{[^{}]*\}\s*validate-json\s*:\s*func\s*\([^()]*\)\s*->\s*validation-report\s*;\s*\}\s*world\s+zorde-validation-guest\s*\{\s*export\s+validation\s*;\s*\}\s*$/.test(wit)) {
    refuse('unexpected package, declaration, operation or world surface');
  }
  // The WIT projection is exact, not a permissive subtype: any unknown
  // constraint can narrow the JSON values while retaining the same fields.
  // The peer TypeSpec and schema comparison remains a separate mandatory gate.
  exactSchemaKeys(jsonSchema,['$schema','$id','$defs'],'root');
  exactSchemaMetadata(jsonSchema,'https://oreslang.dev/schemas/zorde-validation-v1.schema.json');
  const defs=jsonSchema.$defs;
  // The JSON Schema is wider than the exported WIT validation interface:
  // comparison types are legitimate authored schema-only declarations. Do
  // not mistake that for permission to add unreviewed $defs or silently
  // narrow comparison evidence. Admit exactly the five reviewed definitions.
  exactSchemaKeys(defs,
    ['ErrorCode','ComparisonStatus','ValidationIssue','ValidationReport','SchemaComparison'],
    'authored $defs inventory');
  exactSchemaKeys(defs.ComparisonStatus,
    ['$schema','$id','type','enum'], 'ComparisonStatus');
  exactSchemaMetadata(defs.ComparisonStatus,'ComparisonStatus');
  if(defs.ComparisonStatus.type!=='string' ||
     !matchesExactly(defs.ComparisonStatus.enum,['equivalent','different','refused']))
    refuse('authored comparison status values changed');
  exactSchemaKeys(defs.SchemaComparison,
    ['$schema','$id','type','properties','required','unevaluatedProperties'],
    'SchemaComparison');
  exactSchemaMetadata(defs.SchemaComparison,'SchemaComparison');
  exactSchemaFields(defs.SchemaComparison,
    ['schemaVersion','status','authoredDigest','generatedDigest','mismatchPaths'],
    ['schemaVersion','status','authoredDigest','generatedDigest','mismatchPaths']);
  const comparison=defs.SchemaComparison.properties;
  exactSchemaKeys(comparison.schemaVersion,['const'],'SchemaComparison.schemaVersion');
  exactSchemaKeys(comparison.status,['$ref'],'SchemaComparison.status');
  exactSchemaKeys(comparison.authoredDigest,['type'],'SchemaComparison.authoredDigest');
  exactSchemaKeys(comparison.generatedDigest,['type'],'SchemaComparison.generatedDigest');
  exactSchemaKeys(comparison.mismatchPaths,['type','items'],'SchemaComparison.mismatchPaths');
  exactSchemaKeys(comparison.mismatchPaths.items,['type'],'SchemaComparison.mismatchPaths.items');
  if(comparison.schemaVersion.const!==1 ||
     comparison.status.$ref!=='ComparisonStatus' ||
     comparison.authoredDigest.type!=='string' ||
     comparison.generatedDigest.type!=='string' ||
     comparison.mismatchPaths.type!=='array' ||
     comparison.mismatchPaths.items.type!=='string')
    refuse('authored comparison record type drift');
  exactSchemaKeys(defs.ErrorCode,['$schema','$id','type','enum'],'ErrorCode');
  exactSchemaMetadata(defs.ErrorCode,'ErrorCode');
  exactSchemaKeys(defs.ValidationIssue,
    ['$schema','$id','type','properties','required','unevaluatedProperties'],
    'ValidationIssue');
  exactSchemaMetadata(defs.ValidationIssue,'ValidationIssue');
  exactSchemaKeys(defs.ValidationReport,
    ['$schema','$id','type','properties','required','unevaluatedProperties'],
    'ValidationReport');
  exactSchemaMetadata(defs.ValidationReport,'ValidationReport');
  exactSchemaKeys(defs.ValidationIssue.properties?.code,['$ref'],'ValidationIssue.code');
  exactSchemaKeys(defs.ValidationIssue.properties?.path,['type'],'ValidationIssue.path');
  exactSchemaKeys(defs.ValidationReport.properties?.schemaVersion,['const'],
    'ValidationReport.schemaVersion');
  exactSchemaKeys(defs.ValidationReport.properties?.valid,['type'],'ValidationReport.valid');
  exactSchemaKeys(defs.ValidationReport.properties?.issues,['type','items'],
    'ValidationReport.issues');
  exactSchemaKeys(defs.ValidationReport.properties?.issues?.items,['$ref'],
    'ValidationReport.issues.items');
  const cases=tokens(bodyOf(wit,'enum','error-code'));
  if(!matchesExactly(cases,defs.ErrorCode?.enum) ||
     defs.ErrorCode?.type!=='string')
    refuse('error code case or order drift');
  const issue=tokens(bodyOf(wit,'record','validation-issue'));
  if(!matchesExactly(issue,['code: error-code','path: string']))
    refuse('validation issue WIT fields changed');
  exactSchemaFields(defs.ValidationIssue,['code','path'],['code','path']);
  if(defs.ValidationIssue.properties.code?.$ref!=='ErrorCode' ||
     defs.ValidationIssue.properties.path?.type!=='string')
    refuse('validation issue authored schema type drift');
  const report=tokens(bodyOf(wit,'record','validation-report'));
  if(!matchesExactly(report,['schema-version: u32','valid: bool','issues: list<validation-issue>']))
    refuse('validation report WIT fields changed');
  exactSchemaFields(defs.ValidationReport,
    ['schemaVersion','valid','issues'],['schemaVersion','valid','issues']);
  const r=defs.ValidationReport.properties;
  if(r.schemaVersion?.const!==1 || r.valid?.type!=='boolean' ||
     r.issues?.type!=='array' || r.issues.items?.$ref!=='ValidationIssue')
    refuse('validation report authored schema type drift');
  if(!/\bvalidate-json\s*:\s*func\(schema-id:\s*string,\s*payload:\s*list<u8>\)\s*->\s*validation-report\s*;/.test(wit))
    refuse('validate-json boundary type changed');
  if(!/\bworld\s+zorde-validation-guest\s*\{\s*export\s+validation\s*;\s*\}/.test(wit))
    refuse('WIT exported world changed');
  return Object.freeze({status:'STATIC_WIT_CROSSWALK_MATCH',
    nativeGuestExecuted:false,canonicalAbiImplemented:false,fullSchemaParityProven:false});
}

function expectRejected(fn) {
  assert.throws(fn,/ZORDE_WIT_STATIC_CROSSWALK_DENIED/);
}
export function selftest(wit,schema) {
  auditZordeWitProjection(wit,schema);
  let checks=1; // canonical authored WIT and schema actually accepted
  const checkRejected=fn=>{expectRejected(fn);checks++;};
  // Test the byte-level schema-authority admission BEFORE JSON.parse can
  // collapse duplicate members and hide contradictory definitions.
  checkRejected(()=>parseAuthoredZordeSchema('{"$defs":{"x":1,"x":2}}'));
  checkRejected(()=>parseAuthoredZordeSchema(String.raw`{"$defs":{"\\u0061":1,"a":2}}`));
  checkRejected(()=>parseAuthoredZordeSchema('{"x":1} {"x":2}'));
  checkRejected(()=>parseAuthoredZordeSchema('['.repeat(70)+'0'+']'.repeat(70)));
  const replace=(a,b)=> {
    assert.ok(wit.includes(a),'fixture lacks expected anchor '+a);
    return wit.replace(a,b);
  };
  checkRejected(()=>auditZordeWitProjection(replace('valid: bool,','valid: string,'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('missing-required,','missing-required-typo,'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('schema-version: u32,','schema-version: string,'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('payload: list<u8>','payload: string'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('export validation;','import validation;'),schema));
  const changed=structuredClone(schema);
  changed.$defs.ValidationReport.properties.valid.type='string';
  checkRejected(()=>auditZordeWitProjection(wit,changed));
  const changedEnum=structuredClone(schema);
  changedEnum.$defs.ErrorCode.enum[0]='secret-error';
  checkRejected(()=>auditZordeWitProjection(wit,changedEnum));
  // The old permissive regexp admitted trailing world declarations, extra
  // exported methods and package substitutions while matching valid snippets.
  checkRejected(()=>auditZordeWitProjection(replace('@0.1.0;','@0.2.0;'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('  validate-json: func',
    '  leak: func() -> bool;\n  validate-json: func'),schema));
  checkRejected(()=>auditZordeWitProjection(wit+'\nworld injected {}',schema));
  checkRejected(()=>auditZordeWitProjection(replace('  export validation;',
    '  import secret;\n  export validation;'),schema));
  checkRejected(()=>auditZordeWitProjection(replace('  record validation-report {',
    '  record unexpected { leak: string, }\n  record validation-report {'),schema));
  checkRejected(()=>auditZordeWitProjection(wit+' '.repeat(17000),schema));
  // Extra JSON Schema keywords can silently narrow or broaden accepted data.
  const mutateSchema=fn=>{
    const copy=structuredClone(schema);
    fn(copy);
    checkRejected(()=>auditZordeWitProjection(wit,copy));
  };
  mutateSchema(s=>{s.$defs.ValidationReport.properties.valid.const=false;});
  mutateSchema(s=>{s.$defs.ValidationIssue.properties.path.pattern='^[a-z]+$';});
  mutateSchema(s=>{s.$defs.ValidationReport.properties.issues.minItems=1;});
  mutateSchema(s=>{s.$defs.ValidationIssue.properties.code.allOf=[];});
  mutateSchema(s=>{s.$defs.ValidationReport.anyOf=[];});
  mutateSchema(s=>{s.anyOf=[];});
  mutateSchema(s=>{s.$defs.ErrorCode.default='wrong-type';});
  mutateSchema(s=>{s.$defs.ValidationReport.properties.schemaVersion.minimum=0;});
  mutateSchema(s=>{s.$defs.ValidationReport.$id='ValidationReport-v2';});
  mutateSchema(s=>{delete s.$schema;});
  mutateSchema(s=>{s.$defs.ValidationReport.properties.issues.items.$dynamicRef='#next';});
  mutateSchema(s=>{s.$defs.ValidationIssue.properties.path.format='uri';});
  // These two authored comparison definitions do not currently project into
  // WIT, but they are not allowed to change without explicit review.
  mutateSchema(s=>{s.$defs.UnreviewedSecret={type:'string'};});
  mutateSchema(s=>{delete s.$defs.ComparisonStatus;});
  mutateSchema(s=>{s.$defs.ComparisonStatus.enum.push('trusted');});
  mutateSchema(s=>{s.$defs.ComparisonStatus.enum[0]='different';});
  mutateSchema(s=>{s.$defs.ComparisonStatus.title='unreviewed';});
  mutateSchema(s=>{s.$defs.SchemaComparison.properties.status.$ref='Injected';});
  mutateSchema(s=>{s.$defs.SchemaComparison.properties.authoredDigest.maxLength=1;});
  mutateSchema(s=>{s.$defs.SchemaComparison.properties.generatedDigest.pattern='^x$';});
  mutateSchema(s=>{s.$defs.SchemaComparison.properties.mismatchPaths.items.format='email';});
  mutateSchema(s=>{s.$defs.SchemaComparison.unevaluatedProperties=true;});
  mutateSchema(s=>{s.$defs.SchemaComparison.required.pop();});
  mutateSchema(s=>{delete s.$defs.SchemaComparison;});
  return checks; // actual successful checks, not a guessed receipt count
}

if(process.argv[1]&&pathToFileURL(process.argv[1]).href===import.meta.url){
  try{
    const wit=readFileSync('wit/zorde-validation.wit','utf8');
    const schema=parseAuthoredZordeSchema(
      readFileSync('contracts/zorde/authored.schema.json','utf8'));
    const checks=selftest(wit,schema);
    console.log(JSON.stringify({status:'STATIC_WIT_CROSSWALK_MATCH',checks,
      witSyntaxParsed:false,nativeGuestExecuted:false,
      canonicalAbiImplemented:false,releaseReady:false}));
  }catch(e){
    console.error(e?.message||'ZORDE_WIT_STATIC_CROSSWALK_DENIED');
    process.exitCode=1;
  }
}
