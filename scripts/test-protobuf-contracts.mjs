#!/usr/bin/env node
// Conservative contract drift tripwire: NOT TypeSpec compiler evidence,
// JSON Schema validator execution, wasm-tools parsing, or a WIT ABI proof.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const tsp=readFileSync('contracts/protobuf/main.tsp','utf8');
const wit=readFileSync('wit/protobuf-wire.wit','utf8');
const schema=JSON.parse(readFileSync('contracts/protobuf/authored.schema.json','utf8'));
function canonical(s) {
  return s.replace(/\/\/[^\n]*/g,'').replace(/\s+/g,' ').trim();
}
function exactKeys(obj,keys) {
  assert.ok(obj && typeof obj==='object' && !Array.isArray(obj));
  assert.deepEqual(Object.keys(obj).sort(),[...keys].sort());
}
function audit(t,w,j) {
  assert.equal(canonical(t),
    canonical(`
import "@typespec/json-schema";
using TypeSpec.JsonSchema;
namespace Oreslang.Protobuf;
@id("WireField")
model WireField {
  @minValue(1)
  @maxValue(536870911)
  fieldNumber: int32;
  wireType: 0 | 1 | 2 | 5;
  payload: uint8[];
}
@id("WireMessage")
model WireMessage {
  fields: WireField[];
}`),'TypeSpec independent authority changed');

  assert.equal(canonical(w),
    canonical(`
package oreslang:protobuf-wire@0.1.0;
interface protobuf-wire {
  record wire-field {
    field-number: u32,
    wire-type: u8,
    payload: list<u8>,
  }
  record wire-message {
    fields: list<wire-field>,
  }
  record decode-limits {
    max-input-bytes: u32,
    max-fields: u32,
    max-length-delimited-bytes: u32,
  }
  record encode-limits {
    max-output-bytes: u32,
  }
  record codec-error {
    code: string,
    offset: u32,
  }
  decode: func(data: list<u8>, limits: decode-limits) -> result<wire-message, codec-error>;
  encode: func(message: wire-message, limits: encode-limits) -> result<list<u8>, codec-error>;
}
world protobuf-wire-guest {
  export protobuf-wire;
}`),'WIT ABI proposal changed');

  exactKeys(j,['$schema','$id','$defs']);
  assert.equal(j.$schema,'https://json-schema.org/draft/2020-12/schema');
  assert.equal(j.$id,'https://oreslang.dev/schemas/protobuf-wire-envelope-v1.schema.json');
  exactKeys(j.$defs,['WireField','WireMessage']);
  const f=j.$defs.WireField,m=j.$defs.WireMessage;
  for(const [o,id] of [[f,'WireField'],[m,'WireMessage']]) {
    assert.equal(o.$id,id);
    assert.equal(o.$schema,j.$schema);
    assert.equal(o.type,'object');
    assert.equal(o.unevaluatedProperties,false);
    exactKeys(o,['$schema','$id','type','properties','required','unevaluatedProperties']);
  }
  exactKeys(f.properties,['fieldNumber','wireType','payload']);
  assert.deepEqual(f.required,['fieldNumber','wireType','payload']);
  assert.deepEqual(f.properties.fieldNumber,{type:'integer',minimum:1,maximum:536870911});
  assert.deepEqual(f.properties.wireType,{type:'integer',enum:[0,1,2,5]});
  assert.deepEqual(f.properties.payload,{
    type:'array',items:{type:'integer',minimum:0,maximum:255}
  });
  exactKeys(m.properties,['fields']);
  assert.deepEqual(m.required,['fields']);
  assert.deepEqual(m.properties.fields,{type:'array',items:{$ref:'WireField'}});
}
audit(tsp,wit,schema);
let checks=1;
function reject(fn) { assert.throws(fn); checks++; }
const mutant=(mutate)=>{
  const j=structuredClone(schema);
  mutate(j);
  return ()=>audit(tsp,wit,j);
};
reject(mutant(j=>{j.$defs.WireField.properties.fieldNumber.minimum=0;}));
reject(mutant(j=>{j.$defs.WireField.properties.fieldNumber.maximum=536870912;}));
reject(mutant(j=>{j.$defs.WireField.properties.wireType.enum.push(3);}));
reject(mutant(j=>{j.$defs.WireField.properties.payload.items.maximum=256;}));
reject(mutant(j=>{j.$defs.WireField.properties.payload.uniqueItems=true;}));
reject(mutant(j=>{delete j.$defs.WireField.unevaluatedProperties;}));
reject(mutant(j=>{j.$defs.WireMessage.properties.secret={type:'string'};}));
reject(mutant(j=>{j.$defs.WireMessage.properties.fields.items.$ref='Secret';}));
reject(mutant(j=>{j.$defs.WireField.required.pop();}));
reject(mutant(j=>{j.$defs.WireMessage.$id='WireMessageV2';}));
reject(()=>audit(tsp.replace('wireType: 0 | 1 | 2 | 5','wireType: int32'),wit,schema));
reject(()=>audit(tsp.replace('@minValue(1)','@minValue(0)'),wit,schema));
reject(()=>audit(tsp,wit.replace('payload: list<u8>','payload: string'),schema));
reject(()=>audit(tsp,wit.replace('export protobuf-wire','import protobuf-wire'),schema));
reject(()=>audit(tsp,wit+'\nworld unexpected {}',schema));

// The source audit is a narrow lexical tripwire, NOT compilation or an
// executable validator proof. Require the reviewed complete-wire budget path
// and forbid the retired modulo operator in this Protobuf .ores implementation.
const wire=readFileSync('src/protobuf/wire.ores','utf8');
const zorde=readFileSync('src/protobuf/zorde.ores','utf8');
function sourceGuard(w,z) {
  assert.ok(!w.includes('%'), 'unsupported modulo operator in native wire codec');
  assert.ok(w.includes('pub fnc proto_field_reencoded_wire_bytes('),
    'missing full encoded field size API');
  assert.ok(w.includes('proto_unsigned_varint_size(value_bytes)'),
    'missing length-prefix byte accounting');
  assert.ok(z.includes('proto_field_reencoded_wire_bytes(field)'),
    'Zorde fails to count field keys and length prefixes');
  assert.ok(z.includes('options.max_input_bytes - total_reencoded_wire_bytes'),
    'Zorde lacks overflow-safe cumulative budget');
}
sourceGuard(wire,zorde);
checks++;
reject(()=>sourceGuard(wire+'%',zorde));
reject(()=>sourceGuard(wire.replace('pub fnc proto_field_reencoded_wire_bytes(',
  'fnc removed_full_wire_size('),zorde));
reject(()=>sourceGuard(wire.replace('proto_unsigned_varint_size(value_bytes)',
  '0'),zorde));
reject(()=>sourceGuard(wire,zorde.replace('proto_field_reencoded_wire_bytes(field)',
  'proto_field_payload_wire_bytes(field)')));
reject(()=>sourceGuard(wire,zorde.replace('options.max_input_bytes - total_reencoded_wire_bytes',
  'options.max_input_bytes')));
console.log(JSON.stringify({status:'STATIC_PROTOBUF_CONTRACT_SHAPE_MATCH',
  checks,typespecCompilerExecuted:false,jsonSchemaValidatorExecuted:false,
  witParserExecuted:false,nativeGuestExecuted:false,canonicalAbiImplemented:false,
  zordeProofIssued:false,releaseReady:false}));
