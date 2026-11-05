// Host-side static descriptor checks only; no native Zorde guest certification.
import test from 'node:test';
import assert from 'node:assert/strict';
import {descriptorFromSchema} from '../../scripts/zorde-static-contract-ir.mjs';

test('zero-width integer and zero-length string refinements retain distinct canonical kinds',()=>{
  assert.deepEqual(descriptorFromSchema({type:'integer',minimum:0,maximum:0}),
    {version:1,kind:2,property:1,minimum:0,maximum:0,excluded:[]});
  assert.deepEqual(descriptorFromSchema({type:'string',minLength:0,maxLength:0}),
    {version:1,kind:3,property:2,minimum:0,maximum:0,excluded:[]});
});

test('one-point refinements at bounded integer endpoints are admitted without coercion',()=>{
  for(const edge of [-2147483648,2147483647]){
    const value=descriptorFromSchema({type:'integer',minimum:edge,maximum:edge});
    assert.equal(value.minimum,edge);
    assert.equal(value.maximum,edge);
    assert.equal(value.kind,2);
  }
});

test('non-scalar JSON schema extensions cannot silently acquire static authority',()=>{
  for(const schema of [
    {type:'integer',minimum:0,maximum:1,enum:[0]},
    {type:'integer',minimum:0,maximum:1,exclusiveMinimum:0},
    {type:'string',minLength:0,maxLength:2,pattern:'.*'},
    {type:'string',minLength:0,maxLength:2,default:'x'}
  ]) assert.throws(()=>descriptorFromSchema(schema),/ZORDE_STATIC_REFUSED/);
});