#!/usr/bin/env node
/**
 * Static schema/WIT projection guard; deliberately NOT a WIT parser or ABI proof.
 * The independently authored JSON Schema stays authoritative for JSON values.
 * A separate pinned wasm-tools run validates the actual WIT grammar.
 */
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

function refuse(message) { throw new Error('ZORDE_WIT_STATIC_CROSSWALK_DENIED: '+message); }
function matchesExactly(left,right) {
  return Array.isArray(left) && left.length===right.length &&
    left.every((v,i)=>v===right[i]);
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
export function auditZordeWitProjection(rawWit, jsonSchema) {
  if(typeof rawWit!=='string'||!jsonSchema||typeof jsonSchema!=='object')
    refuse('invalid source documents');
  // Reject block comments and WIT annotations until separately modeled.
  // Strip only simple line comments. wasm-tools validates grammar independently.
  if(rawWit.includes('/*')||rawWit.includes('*/')||rawWit.includes('@unstable'))
    refuse('unsupported WIT syntax in static crosswalk');
  const wit=rawWit.replace(/\\/\\/[^\\n]*/g,'');
  const defs=jsonSchema.$defs;
  if(!defs || typeof defs!=='object') refuse('missing authored JSON Schema definitions');
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
  if(!/\\bvalidate-json\\s*:\\s*func\\(schema-id:\\s*string,\\s*payload:\\s*list<u8>\\)\\s*->\\s*validation-report\\s*;/.test(wit))
    refuse('validate-json boundary type changed');
  if(!/\\bworld\\s+zorde-validation-guest\\s*\\{\\s*export\\s+validation\\s*;\\s*\\}/.test(wit))
    refuse('WIT exported world changed');
  return Object.freeze({status:'STATIC_WIT_CROSSWALK_MATCH',
    nativeGuestExecuted:false,canonicalAbiImplemented:false,fullSchemaParityProven:false});
}

function expectRejected(fn) {
  assert.throws(fn,/ZORDE_WIT_STATIC_CROSSWALK_DENIED/);
}
export function selftest(wit,schema) {
  auditZordeWitProjection(wit,schema);
  const replace=(a,b)=> {
    assert.ok(wit.includes(a),'fixture lacks expected anchor '+a);
    return wit.replace(a,b);
  };
  expectRejected(()=>auditZordeWitProjection(replace('valid: bool,','valid: string,'),schema));
  expectRejected(()=>auditZordeWitProjection(replace('missing-required,','missing-required-typo,'),schema));
  expectRejected(()=>auditZordeWitProjection(replace('schema-version: u32,','schema-version: string,'),schema));
  expectRejected(()=>auditZordeWitProjection(replace('payload: list<u8>','payload: string'),schema));
  expectRejected(()=>auditZordeWitProjection(replace('export validation;','import validation;'),schema));
  const changed=structuredClone(schema);
  changed.$defs.ValidationReport.properties.valid.type='string';
  expectRejected(()=>auditZordeWitProjection(wit,changed));
  const changedEnum=structuredClone(schema);
  changedEnum.$defs.ErrorCode.enum[0]='secret-error';
  expectRejected(()=>auditZordeWitProjection(wit,changedEnum));
  return 8; // positive + seven independently checked negative cases
}

if(process.argv[1]&&pathToFileURL(process.argv[1]).href===import.meta.url){
  try{
    const wit=readFileSync('wit/zorde-validation.wit','utf8');
    const schema=JSON.parse(readFileSync('contracts/zorde/authored.schema.json','utf8'));
    const checks=selftest(wit,schema);
    console.log(JSON.stringify({status:'STATIC_WIT_CROSSWALK_MATCH',checks,
      witSyntaxParsed:false,nativeGuestExecuted:false,
      canonicalAbiImplemented:false,releaseReady:false}));
  }catch(e){
    console.error(e?.message||'ZORDE_WIT_STATIC_CROSSWALK_DENIED');
    process.exitCode=1;
  }
}
