import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {auditZordeWitProjection, parseAuthoredZordeSchema, selftest}
  from '../../scripts/audit-zorde-wit-crosswalk.mjs';

const rawWit = readFileSync('wit/zorde-validation.wit','utf8');
const rawSchema = readFileSync('contracts/zorde/authored.schema.json','utf8');

function refuse(mutator) {
  const schema = parseAuthoredZordeSchema(rawSchema);
  mutator(schema);
  assert.throws(()=>auditZordeWitProjection(rawWit,schema),
    /ZORDE_WIT_STATIC_CROSSWALK_DENIED/);
}

test('original five-type authored schema passes the conservative WIT projection', () => {
  const schema = parseAuthoredZordeSchema(rawSchema);
  assert.deepEqual(Object.keys(schema.$defs), [
    'ErrorCode','ComparisonStatus','ValidationIssue','ValidationReport',
    'SchemaComparison'
  ]);
  const result = auditZordeWitProjection(rawWit,schema);
  assert.equal(result.status,'STATIC_WIT_CROSSWALK_MATCH');
  assert.equal(result.nativeGuestExecuted,false);
  assert.equal(result.canonicalAbiImplemented,false);
  assert.equal(selftest(rawWit,schema),43);
});

test('rejects duplicate raw authored schema keys before last-wins JSON.parse', () => {
  for(const raw of [
    '{"$defs":{"x":1,"x":2}}',
    String.raw`{"$defs":{"\\u0061":1,"a":2}}`,
    '{"$defs":{"x":{"v":1,"v":2}}}',
    '{"x":1}{"x":2}',
  ]) {
    assert.throws(()=>parseAuthoredZordeSchema(raw),
      /ZORDE_WIT_STATIC_CROSSWALK_DENIED/);
  }
});

test('rejects unknown, missing or narrowed comparison-only definitions',()=>{
  refuse(s=>{s.$defs.Leak={type:'string'};});
  refuse(s=>{delete s.$defs.ComparisonStatus;});
  refuse(s=>{s.$defs.ComparisonStatus.enum[1]='unknown';});
  refuse(s=>{s.$defs.ComparisonStatus.enum.push('trusted');});
  refuse(s=>{s.$defs.SchemaComparison.properties.authoredDigest.maxLength=8;});
  refuse(s=>{s.$defs.SchemaComparison.properties.generatedDigest.pattern='^bad$';});
  refuse(s=>{s.$defs.SchemaComparison.properties.status.$ref='ValidationReport';});
  refuse(s=>{s.$defs.SchemaComparison.properties.mismatchPaths.items.format='uri';});
  refuse(s=>{s.$defs.SchemaComparison.unevaluatedProperties=true;});
  refuse(s=>{s.$defs.SchemaComparison.required.pop();});
});
