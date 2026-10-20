# Synthetic receipt-signer regression: no private compiler/source or network needed.
from pathlib import Path
import hashlib, json, os, subprocess, tempfile, shutil

script=(Path(__file__).resolve().parents[2]/'scripts/test-zorde-contract-source-local.sh').read_text()
block=script.rsplit("python3 - <<'PY'",1)[1].split('\nPY\n',1)[0].split('\n',1)[1]
assert 'tap_metric' in block and 'expected_sources' in block
sha=lambda b:hashlib.sha256(b).hexdigest()
root=Path(tempfile.mkdtemp(prefix='zorde-static-signer-',dir='/tmp'))
evidence=root.parent/(root.name+'-evidence')
evidence.mkdir(mode=0o700)
paths={
 'typespec':'contracts/zorde/scalars.tsp',
 'authoredJsonSchema':'contracts/zorde/scalars.schema.json',
 'reviewedParity':'artifacts/zorde-scalars-parity.json',
 'contractIr':'artifacts/zorde-scalars-contract-ir.json',
 'consumerVerification':'artifacts/zorde-scalars-consumer-verification.json',
 'guestPlan':'artifacts/zorde-generated/registry-plan.json',
 'guestSpecs':'artifacts/zorde-generated/registry-specs.ores',
 'guestSmoke':'artifacts/zorde-generated/registry-smoke.ores',
 'buildComplete':'artifacts/zorde-generated/build-complete.json',
}
inputs=['tests/source_free/test_zorde_contract_ir.mjs','tests/source_free/test_zorde_native_guard.mjs','scripts/zorde-static-contract-ir.mjs']
source='a'*40;tree='b'*40;node='c'*64
runenv=os.environ.copy();runenv.update(SOURCE=source,TREE=tree,TJSV='d'*40,TJSV_TREE='e'*40,NODE_SHA=node,EVIDENCE=str(evidence))
def write(p,b):
 p=Path(p);p.parent.mkdir(parents=True,exist_ok=True)
 p.write_bytes(b if isinstance(b,bytes) else str(b).encode());return p
try:
 write(root/'scripts/test-zorde-contract-source-local.sh',script)
 for p in inputs:write(root/p, 'unmodified synthetic input '+p+'\n')
 corpus='f'*64
 obj={
 'reviewedParity':{'status':'passed','runId':'verified-tjsv-run','zeroUnexplainedFindings':True,'configuration':{'mode':'check','differential':{'instanceCorpusDigest':corpus}},'coverage':{'sourceMutationCheck':True},'differential':{'summary':{'corpusInstances':9,'matchedReviewedCorpusInstances':9,'reviewedUnassertedInstances':0,'declarationsWithReviewedPositiveAndNegative':2,'divergences':0,'refusals':0}}},
 'contractIr':{'irId':'synthetic-IR','status':'passed','admissible':True},
 'consumerVerification':{'status':'passed','admissible':True,'computedIrId':'synthetic-IR','suppliedIrId':'synthetic-IR','expectedIrId':'synthetic-IR'},
 'guestPlan':{'schema':'oreslang.zorde.static-scalar-registry-plan/v1','role':'non-authoritative-generated-build-artifact','irId':'synthetic-IR','tjsvRunId':'verified-tjsv-run','runtimeImplementationVerified':False,'actorSafeSharingVerified':False,'releaseReady':False},
 'buildComplete':{'schema':'oreslang.zorde.static-registry-build-complete/v1','irId':'synthetic-IR','tjsvRunId':'verified-tjsv-run','releaseReady':False,'nativeGuestExecuted':False,'reviewedCorpusDigest':corpus,'reviewedCorpusInstances':9,'guestSmokeCompiled':False,'guestSmokeExecuted':False}}
 for k,p in paths.items():write(root/p,json.dumps(obj[k]) if k in obj else 'synthetic '+k+'\n')
 built=obj['buildComplete']
 built['planSha256']=sha((root/paths['guestPlan']).read_bytes())
 built['guestSha256']=sha((root/paths['guestSpecs']).read_bytes())
 built['smokeSha256']=sha((root/paths['guestSmoke']).read_bytes())
 write(root/paths['buildComplete'],json.dumps(built))
 baseline={p:(root/p).read_bytes() for p in paths.values()}
 baseline.update({p:(root/p).read_bytes() for p in inputs})
 tap='TAP version 13\n# tests 90\n# pass 90\n# fail 0\n# skipped 0\n# todo 0\n# cancelled 0\n'
 log=evidence/'zorde-node-real-suite.log';write(log,tap)
 write(evidence/'zorde-contract-static.log','Synthetic static contract run output\n')
 def receipt():
  return {'schema':'oreslang.zorde.source-local.node-security-suite/v1','source_head_sha':source,'source_root_tree_sha':tree,'node_executable_sha256':node,'real_node_crypto_filesystem_tests_executed':True,'release_ready':False,'source_free_test_cases_passed':90,'source_free_test_cases_failed':0,'source_free_test_cases_skipped':0,'node_test_log_sha256':sha(log.read_bytes()),'test_source_sha256':{p:sha((root/p).read_bytes()) for p in inputs[:2]},'source_module_sha256':{inputs[2]:sha((root/inputs[2]).read_bytes())}}
 nr=evidence/'zorde-node-real-suite-receipt.json';base=receipt()
 def check(label,edit=None,expect=True):
  for filename,data in baseline.items():write(root/filename,data)
  write(log,tap)
  write(nr,json.dumps(base))
  if edit:edit()
  (evidence/'zorde-contract-static-receipt.json').unlink(missing_ok=True)
  p=subprocess.run(['python3','-c',block],cwd=root,env=runenv,capture_output=True,text=True,timeout=10)
  good=p.returncode==0 and (evidence/'zorde-contract-static-receipt.json').is_file()
  if good!=expect:raise AssertionError(f'{label}: expected={expect}, got={good}, stderr={p.stderr[-500:]}')
  print(f'{label}: '+('PASS receipt issued' if expect else 'DENIED no receipt'))
  if not expect: assert not (evidence/'zorde-contract-static-receipt.json').exists()
  if expect:
   z=json.loads((evidence/'zorde-contract-static-receipt.json').read_text())
   assert z['release_ready'] is False and z['native_guest_executed'] is False
   assert z['static_execution_log_sha256']==sha((evidence/'zorde-contract-static.log').read_bytes())
 check('positive synthetic provenance',expect=True)
 check('forged child pass count',lambda: write(nr,json.dumps({**base,'source_free_test_cases_passed':89})),False)
 def todo():
  modified=tap.replace('# pass 90','# pass 89').replace('# todo 0','# todo 1')
  write(log,modified);write(nr,json.dumps({**base,'source_free_test_cases_passed':89,'node_test_log_sha256':sha(log.read_bytes())}))
 check('forged TODO masked by child receipt',todo,False)
 def tamper():write(root/inputs[2], 'synthetic modified compiler bridge!')
 check('changed compiler bridge',tamper,False)
 write(root/inputs[2],'unmodified synthetic input '+inputs[2]+'\n')
 def omit():
  b=base.copy(); b.pop('source_module_sha256');write(nr,json.dumps(b))
 check('omitted module fingerprints',omit,False)
 def swap():
  b=base.copy();b['test_source_sha256']={'unexpected': '0'*64};write(nr,json.dumps(b))
 check('substituted source fingerprint map',swap,False)
 def corrupt_build(field,val):
  p=root/paths['buildComplete']; v=json.loads(p.read_text());v[field]=val;write(p,json.dumps(v))
 check('tampered plan digest',lambda: corrupt_build('planSha256','0'*64),False)
 check('tampered guest source after generation',lambda: write(root/paths['guestSpecs'],'attacker changed guest source'),False)
 check('divergent TJSV run id',lambda: corrupt_build('tjsvRunId','forged-run'),False)
 def missing_negative_coverage():
  p=root/paths['reviewedParity'];v=json.loads(p.read_text());v['differential']['summary']['declarationsWithReviewedPositiveAndNegative']=1;write(p,json.dumps(v))
 check('missing positive-and-negative corpus assertions',missing_negative_coverage,False)
 def forged_ir_status():
  p=root/paths['contractIr'];v=json.loads(p.read_text());v['admissible']=False;write(p,json.dumps(v))
 check('inadmissible IR status',forged_ir_status,False)
 print('ZORDE_STATIC_SIGNER_SYNTHETIC_11_OF_11_PASSED: no private source or compiler invoked')
finally:
 shutil.rmtree(root);shutil.rmtree(evidence)
