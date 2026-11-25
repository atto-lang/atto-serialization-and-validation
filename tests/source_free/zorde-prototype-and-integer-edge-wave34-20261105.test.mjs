// Source-backed host descriptor tests only: no native Zorde runtime/certification.
import test from 'node:test';
import assert from 'node:assert/strict';
import {descriptorFromSchema} from '../../scripts/zorde-static-contract-ir.mjs';

test('int32 lower/upper endpoints admit only own finite bounded safe integers',()=>{
  assert.deepEqual(descriptorFromSchema({type:'integer',minimum:-2147483648,maximum:2147483647}),
    {version:1,kind:2,property:1,minimum:-2147483648,maximum:2147483647,excluded:[]});
  for(const [min,max] of [
    [-2147483649,0],[0,2147483648],[2,1],
    [NaN,3],[0,Infinity],[true,1],['0',1],[0,1n],[0.125,2],
  ]) assert.throws(()=>descriptorFromSchema({type:'integer',minimum:min,maximum:max}),/ZORDE_STATIC_REFUSED/);
});

test('inherited type and bounds do not elevate a schema to static authority',()=>{
  const inheritedType=Object.assign(Object.create({type:'integer'}),{minimum:0,maximum:1});
  assert.throws(()=>descriptorFromSchema(inheritedType),/ZORDE_STATIC_REFUSED/);
  const inheritedBounds=Object.assign(Object.create({minimum:0,maximum:1}),{type:'integer'});
  assert.throws(()=>descriptorFromSchema(inheritedBounds),/ZORDE_STATIC_REFUSED/);
  assert.throws(()=>descriptorFromSchema([]),/ZORDE_STATIC_REFUSED/);
  assert.throws(()=>descriptorFromSchema(null),/ZORDE_STATIC_REFUSED/);
});

test('length-constrained scalar must reject unsupported and coercive extensions',()=>{
  const input=Object.assign(Object.create(null),{type:'string',minLength:0,maxLength:65536});
  assert.deepEqual(descriptorFromSchema(input),
    {version:1,kind:3,property:2,minimum:0,maximum:65536,excluded:[]});
  for(const extra of [{pattern:'.*'},{default:'x'},{enum:['x']},{minLength:'0'},
                      {maxLength:65537},{maxLength:-1},{minLength:2,maxLength:1}]) {
    assert.throws(()=>descriptorFromSchema({...input,...extra}),/ZORDE_STATIC_REFUSED/);
  }
});
