import { createHash } from "node:crypto";

// Candidate, intentionally limited to the currently implemented wire profiles.
const kinds = {
  cbor: new Set(["bool","i64","f64","text","list","record"]),
  protobuf: new Set(["varint","fixed32","fixed64","bytes","message","packed"]),
};
const ascii = (s) => typeof s === "string" && s.length>=1 && s.length<=128 && !/[^A-Za-z0-9_.:-]/.test(s);
const uint = (n,max) => Number.isSafeInteger(n) && n>=0 && n<=max;
const require = (ok) => { if(!ok) throw new Error("invalid schema descriptor"); };
const keys = (o,expected) => {
  require(o && typeof o==="object" && !Array.isArray(o));
  const names=expected.split(",");require(Object.keys(o).length===names.length && names.every(k=>Object.hasOwn(o,k)));
};
const cmp = (a,b)=>a<b?-1:a>b?1:0;
export function canonicalShape(d) {
  keys(d,"codec,version,root,nodes");
  require(typeof d.codec==="string" && Object.hasOwn(kinds,d.codec) && uint(d.version,2147483647) && d.version>0 && ascii(d.root));
  require(Array.isArray(d.nodes) && d.nodes.length>0 && d.nodes.length<=256);
  const table=new Map();
  let fieldCount=0;
  for(const n of d.nodes) {
    keys(n,"id,kind,fields");
    require(ascii(n.id) && kinds[d.codec].has(n.kind) && !table.has(n.id));
    require(Array.isArray(n.fields) && n.fields.length<=1024);
    fieldCount+=n.fields.length;require(fieldCount<=4096);
    const fields=[], names=new Set(), slots=new Set();
    for(const f of n.fields) {
      keys(f,"slot,name,target");
      require(uint(f.slot,536870911) && ascii(f.name) && ascii(f.target)
              && !names.has(f.name) && !slots.has(f.slot));
      if(d.codec==="protobuf" && n.kind==="message")
        require(f.slot>0 && !(f.slot>=19000 && f.slot<=19999));
      names.add(f.name);slots.add(f.slot);fields.push(f);
    }
    fields.sort((a,b)=>a.slot-b.slot);
    if(n.kind==="record") require(fields.every((f,i)=>f.slot===i));
    else if(n.kind==="list" || n.kind==="packed")
      require(fields.length===1 && fields[0].slot===0 && fields[0].name==="element");
    else if(n.kind!=="message") require(fields.length===0);
    table.set(n.id,{kind:n.kind,fields});
  }
  require(table.has(d.root));
  for(const n of table.values())for(const f of n.fields) {
    require(table.has(f.target));
    if(n.kind==="packed") require(["varint","fixed32","fixed64"].includes(table.get(f.target).kind));
  }
  const seen=new Set(), work=[d.root];
  while(work.length){const id=work.pop();if(seen.has(id))continue;seen.add(id);for(const f of table.get(id).fields)work.push(f.target);}
  require(seen.size===table.size);
  // All text is restricted ASCII, all numbers are canonical decimal; no escaping ambiguity.
  const lines=["ATTO-SHAPE-1",d.codec,String(d.version),d.root,String(table.size)];
  for(const id of [...table.keys()].sort(cmp)) {
    const n=table.get(id);lines.push(id,n.kind,String(n.fields.length));
    for(const f of n.fields)lines.push(String(f.slot),f.name,f.target);
  }
  return Buffer.from(lines.join("\n")+"\n","ascii");
}
export function shapeDigest(d) {return createHash("sha256").update(canonicalShape(d)).digest("hex");}
