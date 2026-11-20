// Static host descriptor preflight only; no signed producer, guest or codec runtime claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import {descriptorFromSchema} from '../../scripts/zorde-static-contract-ir.mjs';

test('admitted annotations do not alter exact signed int32 scalar descriptor', () => {
  const base={type:'integer',format:'int32',minimum:-7,maximum:7};
  const expected=descriptorFromSchema(base);
  assert.deepEqual(expected,{version:1,kind:2,property:1,minimum:-7,maximum:7,excluded:[]});
  const annotated={...base,$id:'urn:test:int',title:'Signed integer',
    description:'Source annotation only', $schema:'https://json-schema.org/draft/2020-12/schema'};
  assert.deepEqual(descriptorFromSchema(annotated),expected);
});

test('schema extensions cannot smuggle unaudited scalar restrictions', () => {
  const base={type:'integer',minimum:0,maximum:8};
  for(const extra of [
    {const:1},{enum:[0,1]},{default:1},{multipleOf:2},{exclusiveMaximum:8},
    {nullable:true},{oneOf:[{type:'integer'}]},{$ref:'#/$defs/Scalar'},
    {properties:{}},{additionalProperties:false},{minLength:1},
    {format:'int64'},{$schema:'https://json-schema.org/draft/2019-09/schema'}
  ]) {
    assert.throws(()=>descriptorFromSchema({...base,...extra}),/ZORDE_STATIC_REFUSED/);
  }
});

test('inherited scalar bounds never replace own bounded identity', () => {
  const inherited=Object.create({minimum:0,maximum:8});
  inherited.type='integer';
  assert.throws(()=>descriptorFromSchema(inherited),/ZORDE_STATIC_REFUSED/);
  const inheritedKind=Object.create({type:'integer'});
  inheritedKind.minimum=0;
  inheritedKind.maximum=8;
  assert.throws(()=>descriptorFromSchema(inheritedKind),/ZORDE_STATIC_REFUSED/);
});

test('null-prototype own descriptor and Unicode annotation are admitted without type coercion', () => {
  const input=Object.assign(Object.create(null),
    {type:'string',minLength:1,maxLength:3,title:'μ😀'});
  assert.deepEqual(descriptorFromSchema(input),
    {version:1,kind:3,property:2,minimum:1,maximum:3,excluded:[]});
  for(const bounds of [{minLength:'1',maxLength:3},{minLength:1,maxLength:3n},
                       {minLength:1.5,maxLength:3},{minLength:4,maxLength:3}]) {
    assert.throws(()=>descriptorFromSchema({type:'string',...bounds}),/ZORDE_STATIC_REFUSED/);
  }
});
