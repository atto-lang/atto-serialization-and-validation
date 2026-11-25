import json
from pathlib import Path
from actor_data_shape_digest_v1 import canonical_shape,shape_digest
cases=json.loads(Path(__file__).with_name("actor_data_shape_digest_vectors_v1.json").read_text())
results={}
for c in cases:
    if not c["valid"]:
        try: canonical_shape(c["descriptor"])
        except ValueError: continue
        raise AssertionError(c["name"])
    digest=shape_digest(c["descriptor"])
    if "equal" in c: assert digest==results[c["equal"]]["digest"]
    if "different" in c: assert digest!=results[c["different"]]["digest"]
    results[c["name"]]={"digest":digest,"canonical":canonical_shape(c["descriptor"]).hex()}
print(json.dumps(results,separators=(",",":")))
