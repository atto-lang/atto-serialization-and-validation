"""Independent candidate canonical shape reference, restricted wire profiles."""
import hashlib
import re

KINDS = {"cbor": {"bool","i64","f64","text","list","record"},
         "protobuf": {"varint","fixed32","fixed64","bytes","message","packed"}}
def require(ok):
    if not ok:
        raise ValueError("invalid schema descriptor")
def token(s):
    return isinstance(s,str) and re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}",s) is not None
def uint(n,m):
    return type(n) in (int,float) and 0 <= n <= m and int(n)==n
def keys(o,names):
    require(type(o) is dict and set(o)==set(names.split(",")))
def canonical_shape(d):
    keys(d,"codec,version,root,nodes")
    require(isinstance(d["codec"],str) and d["codec"] in KINDS and uint(d["version"],2147483647)
            and d["version"]>0 and token(d["root"]))
    require(type(d["nodes"]) is list and 0<len(d["nodes"])<=256)
    table={}
    total=0
    for n in d["nodes"]:
        keys(n,"id,kind,fields")
        require(token(n["id"]) and isinstance(n["kind"],str) and n["kind"] in KINDS[d["codec"]] and n["id"] not in table)
        require(type(n["fields"]) is list and len(n["fields"])<=1024)
        total+=len(n["fields"])
        require(total<=4096)
        names=set()
        slots=set()
        for f in n["fields"]:
            keys(f,"slot,name,target")
            require(uint(f["slot"],536870911) and token(f["name"]) and token(f["target"])
                    and f["name"] not in names and f["slot"] not in slots)
            if d["codec"]=="protobuf" and n["kind"]=="message":
                require(f["slot"]>0 and not 19000<=f["slot"]<=19999)
            names.add(f["name"])
            slots.add(f["slot"])
        fields=sorted(n["fields"],key=lambda f:f["slot"])
        if n["kind"]=="record":
            require(all(f["slot"]==i for i,f in enumerate(fields)))
        elif n["kind"] in ("list","packed"):
            require(len(fields)==1 and fields[0]["slot"]==0 and fields[0]["name"]=="element")
        elif n["kind"]!="message":
            require(not fields)
        table[n["id"]]=(n["kind"],fields)
    require(d["root"] in table)
    for kind,fields in table.values():
        for f in fields:
            require(f["target"] in table)
            if kind=="packed":
                require(table[f["target"]][0] in ("varint","fixed32","fixed64"))
    seen=set()
    work=[d["root"]]
    while work:
        node=work.pop()
        if node in seen: continue
        seen.add(node)
        work.extend(f["target"] for f in table[node][1])
    require(len(seen)==len(table))
    lines=["ATTO-SHAPE-1",d["codec"],str(int(d["version"])),d["root"],str(len(table))]
    for node in sorted(table):
        kind,fields=table[node]
        lines.extend([node,kind,str(len(fields))])
        for f in fields:
            lines.extend([str(int(f["slot"])),f["name"],f["target"]])
    return ("\n".join(lines)+"\n").encode("ascii")
def shape_digest(d):
    return hashlib.sha256(canonical_shape(d)).hexdigest()
