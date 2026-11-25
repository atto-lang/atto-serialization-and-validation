import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {canonicalShape,shapeDigest} from "./actor_data_shape_digest_v1.mjs";
const cases=JSON.parse(readFileSync(new URL("./actor_data_shape_digest_vectors_v1.json",import.meta.url),"utf8"));
const results={};
for(const c of cases){
  if(!c.valid){assert.throws(()=>canonicalShape(c.descriptor));continue;}
  const digest=shapeDigest(c.descriptor);
  if(c.equal)assert.equal(digest,results[c.equal].digest);
  if(c.different)assert.notEqual(digest,results[c.different].digest);
  results[c.name]={digest,canonical:canonicalShape(c.descriptor).toString("hex")};
}
console.log(JSON.stringify(results));
