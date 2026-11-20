// Source-free schema admission tests. Not native Zorde registry / guest proof.
import test from 'node:test';
import assert from 'node:assert/strict';
import {descriptorFromSchema} from '../../scripts/zorde-static-contract-ir.mjs';

test('int32 bounds accept exact signed endpoints without changing type identity',()=>{
 const x=descriptorFromSchema({type:'integer',format:'int32',minimum:-2147483648,maximum:2147483647});
 assert.deepEqual(x,{version:1,kind:2,property:1,minimum:-2147483648,maximum:2147483647,excluded:[]});
 assert.deepEqual(descriptorFromSchema({type:'integer',minimum:0,maximum:0}),
   {version:1,kind:2,property:1,minimum:0,maximum:0,excluded:[]});
});

test('integer constraints forbid unsafe, fractional, inverted and missing bounds',()=>{
 for(const [minimum,maximum] of [[-2147483649,0],[0,2147483648],
   [0,0.5],[Infinity,1],[NaN,1],[10,1],[-0.5,1]]){
  assert.throws(()=>descriptorFromSchema({type:'integer',minimum,maximum}),/ZORDE_STATIC_REFUSED/);
 }
 for(const partial of [{type:'integer',maximum:1},{type:'integer',minimum:1}])
  assert.throws(()=>descriptorFromSchema(partial),/ZORDE_STATIC_REFUSED/);
});

test('string length uses exact integer code-point bounds and rejects implicit policy',()=>{
 const max=descriptorFromSchema({type:'string',minLength:0,maxLength:65536});
 assert.equal(max.kind,3);
 assert.equal(max.property,2);
 assert.equal(max.maximum,65536);
 for(const [minLength,maxLength] of [[1,0],[0,65537],[-1,2],[0,1.1],[0,Infinity]]){
  assert.throws(()=>descriptorFromSchema({type:'string',minLength,maxLength}),/ZORDE_STATIC_REFUSED/);
 }
});

test('unsupported keywords and dialects never silently weaken authority',()=>{
 const base={type:'integer',minimum:1,maximum:9};
 for(const poison of [
  {default:5},{'x-coerce':true},{multipleOf:2},{format:'int64'},
  {$schema:'http://json-schema.org/draft-07/schema#'},{exclusiveMinimum:1}
 ]) assert.throws(()=>descriptorFromSchema({...base,...poison}),/ZORDE_STATIC_REFUSED/);
 assert.throws(()=>descriptorFromSchema(Object.create({type:'integer',minimum:1,maximum:2})),/ZORDE_STATIC_REFUSED/);
 assert.throws(()=>descriptorFromSchema(null),/ZORDE_STATIC_REFUSED/);
});
