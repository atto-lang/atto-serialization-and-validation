import test from 'node:test';
import assert from 'node:assert/strict';
import {deriveStaticRegistry, descriptorFromSchema, emitOreslangSpecs, rejectDuplicateJsonKeys,
  canonicalDigest,semanticReportProjection,readEvidenceFile,emitZordePilotGuestSmoke,
  reviewedCorpusIdentity} from
  '../../scripts/zorde-static-contract-ir.mjs';

import {createHash} from 'node:crypto';
import {mkdtempSync,writeFileSync,readFileSync,symlinkSync,linkSync,rmSync,mkdirSync,
  existsSync,constants as fsConstants} from 'node:fs';
import {join,resolve} from 'node:path';
import {realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {spawnSync} from 'node:child_process';

const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const h = (c) => c.repeat(64);
function fixture() {
  const report = {
    schema:'ores.typespec-json-schema-validator.report/v1',
    runId:h('a'),status:'passed',zeroUnexplainedFindings:true,findings:[],
    configuration:{mode:'check',emitterOptions:null,executionMode:null,
      differential:{enabled:true,instanceCorpus:null,instanceCorpusDigest:h('9')}},
    toolchain:{validator:{name:'@oresoftware/typespec-json-schema-validator',version:'0.1.1'},
      typespecCompiler:{version:'1.16.0'},jsonSchemaEmitter:'@typespec/json-schema'},
    counts:{findings:0,structuralFindings:0,differentialFindings:0,
      emittedFindings:0,findingsTruncated:false},
    coverage:{sourceMutationCheck:true,directDeclarationInventory:true,
      typespecGeneratedJsonSchemaComparison:true,differentialInstanceValidation:true},
    differential:{
      summary:{probesEvaluated:22,divergences:0,refusals:0,
        comparedDeclarations:1,corpusInstances:3,matchedReviewedCorpusInstances:3,
        reviewedAcceptedInstances:2,reviewedRejectedInstances:1,
        reviewedUnassertedInstances:0,
        declarationsWithReviewedPositiveAndNegative:1,
        declarationsWithoutReviewedFixtures:0},
      declarations:[{
        typespec:'Zorde.Validation.Port',generated:'Port',authored:'Port',
        probes:22,divergences:0,refusals:0,behaviorallyIndistinguishable:true,
        reviewedCorpus:{accepted:2,rejected:1,unasserted:0,total:3},
        reviewedFixtureCoverage:'positive-and-negative',
      }],

    },
    inputs:{typespec:{digest:h('b')},authoredJsonSchema:{digest:h('c')},
      generatedJsonSchema:{digest:h('d')}},
    declarationMap:[{typespec:'Zorde.Validation.Port',kind:'scalar',generated:'Port',authored:'Port'}],
  };
  const port = {type:'integer',minimum:1,maximum:65535};
  const ir = {
    schema:'ores.typespec-json-schema-validator.contract-ir/v1',
    irId:h('e'),status:'passed',admissible:true,
    role:'downstream-derived-parity-artifact',editableAuthority:false,
    authorities:{typespec:'independently-authored',jsonSchema:'independently-authored',
      generatedJsonSchema:'comparison-evidence-only',precedence:'none'},
    admission:{
      receipt:{schema:report.schema,runId:report.runId,digest:h('f'),
        status:'passed',zeroUnexplainedFindings:true},
      requirements:{exactInputDigests:true,directDeclarationInventory:true,
        generatedSchemaComparison:true,differentialInstanceValidation:true,
        zeroUnexplainedFindings:true},
      scope:{admittedDeclarations:1,excludedDeclarations:0,
        outOfScopeDeclarations:0,complete:true}
    },
    provenance:{
      typespec:{digest:report.inputs.typespec.digest},
      generatedJsonSchema:{digest:report.inputs.generatedJsonSchema.digest},
      authoredJsonSchema:{digest:report.inputs.authoredJsonSchema.digest},
    },
    excludedDeclarations:[],outOfScopeDeclarations:[],
    declarations:[{
      id:'Zorde.Validation.Port',kind:'scalar-like',
      names:{typespec:'Zorde.Validation.Port',generatedJsonSchema:'Port',
        authoredJsonSchema:'Port'},
      assertionSchema:port,assertionDigest:h('1'),
      lanes:{
        typespecGeneratedJsonSchema:{role:'comparison-evidence-only',
          schemaDigest:h('2'),normalizedSchema:structuredClone(port)},
        authoredJsonSchema:{role:'independently-authored-authority',
          schemaDigest:h('3'),normalizedSchema:structuredClone(port)},
      },
    }],
  };
  const identity=semanticReportProjection(report);
  report.runId=canonicalDigest({
    schema:report.schema,status:report.status,
    configuration:identity.configuration,
    inputDigests:{typespec:report.inputs.typespec.digest,
      authored:report.inputs.authoredJsonSchema.digest,
      generated:report.inputs.generatedJsonSchema.digest},
    toolchain:identity.toolchain,
    findings:[],
  });
  ir.admission.receipt.runId=report.runId;
  const d=ir.declarations[0];
  d.assertionDigest=canonicalDigest(d.assertionSchema);
  d.lanes.typespecGeneratedJsonSchema.schemaDigest=
    canonicalDigest(d.lanes.typespecGeneratedJsonSchema.normalizedSchema);
  d.lanes.authoredJsonSchema.schemaDigest=
    canonicalDigest(d.lanes.authoredJsonSchema.normalizedSchema);
  ir.admission.receipt.digest=canonicalDigest(semanticReportProjection(report));
  const {irId:_,...body}=ir;
  ir.irId=canonicalDigest(body);
  return {ir,report};
}
function refuse(change) {
  const {ir,report}=fixture();
  change(ir,report);
  assert.throws(()=>deriveStaticRegistry(ir,report),/ZORDE_STATIC_REFUSED/);
}

test('valid derived scalar plan is deterministic; guest code has bounded typed metadata',()=>{
  const {ir,report}=fixture();
  const plan=deriveStaticRegistry(ir,report);
  assert.equal(plan.entries.length,1);
  assert.deepEqual(plan.entries[0],{schemaId:'Zorde.Validation.Port',
    descriptor:{version:1,kind:2,property:1,minimum:1,maximum:65535,excluded:[]}});
  assert.equal(plan.releaseReady,false);
  assert.equal(plan.actorSafeSharingVerified,false);
  const source=emitOreslangSpecs(plan);
  assert.match(source,/zorde_contract_registry_specs/);
  assert.match(source,/new ZordeScalarDescriptor\(1,2,1,1,65535,new List<int>\(\)\)/);
  assert.equal(emitOreslangSpecs(deriveStaticRegistry(ir,report)),source);
});
test('bounded Unicode scalar length is an admitted descriptor, without string coercion',()=>{
  assert.deepEqual(descriptorFromSchema({
    type:'string',minLength:1,maxLength:64
  }),{version:1,kind:3,property:2,minimum:1,maximum:64,excluded:[]});
});

test('emitter rejects forged plans and source injection even when called directly',()=>{
  const {ir,report}=fixture();
  const valid=deriveStaticRegistry(ir,report);
  const evil=structuredClone(valid);
  evil.entries[0].schemaId='Port"); panic(); //';
  assert.throws(()=>emitOreslangSpecs(evil),/ZORDE_STATIC_REFUSED/);
  const widened=structuredClone(valid);
  widened.entries[0].descriptor.maximum=Number.MAX_SAFE_INTEGER;
  assert.throws(()=>emitOreslangSpecs(widened),/ZORDE_STATIC_REFUSED/);
  const dishonest=structuredClone(valid);
  dishonest.releaseReady=true;
  assert.throws(()=>emitOreslangSpecs(dishonest),/ZORDE_STATIC_REFUSED/);
});

test('generated scalar plans refuse missing, forged or extra authority digests',()=>{
  const {ir,report}=fixture();
  const plan=deriveStaticRegistry(ir,report);
  const missing=structuredClone(plan);
  delete missing.sourceDigests.typespec;
  assert.throws(()=>emitOreslangSpecs(missing),/ZORDE_STATIC_REFUSED/);
  const malformed=structuredClone(plan);
  malformed.sourceDigests.authoredJsonSchema='0'.repeat(63);
  assert.throws(()=>emitOreslangSpecs(malformed),/ZORDE_STATIC_REFUSED/);
  const injected=structuredClone(plan);
  injected.sourceDigests.unreviewed='f'.repeat(64);
  assert.throws(()=>emitOreslangSpecs(injected),/ZORDE_STATIC_REFUSED/);
  const blank=structuredClone(plan);
  blank.sourceDigests=null;
  assert.throws(()=>emitOreslangSpecs(blank),/ZORDE_STATIC_REFUSED/);
  assert.equal(emitOreslangSpecs(plan),emitOreslangSpecs(deriveStaticRegistry(ir,report)));
});

test('digest-gated input reader refuses decoded duplicate JSON keys',()=>{
  rejectDuplicateJsonKeys('{"role":1,"nested":{"value":true},"list":[{"a":1},{"a":2}]}');
  assert.throws(()=>rejectDuplicateJsonKeys('{"role":1,"role":2}'),
    /ZORDE_STATIC_REFUSED/);
  assert.throws(()=>rejectDuplicateJsonKeys(String.raw`{"role":1,"\u0072ole":2}`),
    /ZORDE_STATIC_REFUSED/);
  assert.throws(()=>rejectDuplicateJsonKeys('{"a":{"x":1,"x":2}}'),
    /ZORDE_STATIC_REFUSED/);
  assert.throws(()=>rejectDuplicateJsonKeys('{"a":1,"b":[{"x":1,"x":2}]}'),
    /ZORDE_STATIC_REFUSED/);
  assert.throws(()=>rejectDuplicateJsonKeys('['.repeat(70)+'0'+']'.repeat(70)),
    /ZORDE_STATIC_REFUSED/);
  assert.throws(()=>rejectDuplicateJsonKeys('{"a":1} {"b":2}'),
    /ZORDE_STATIC_REFUSED/);
});

test('inherited prototype constraints must not substitute for authored fields',()=>{
  const forged=Object.create({minimum:1});
  forged.type='integer';forged.maximum=3;
  assert.throws(()=>descriptorFromSchema(forged),/ZORDE_STATIC_REFUSED/);
});

// Deterministic property probes exercise lowering, not native guest execution.
test('5,000 randomized int32 intervals preserve exact bounds or refuse narrowing',()=>{
  let seed=0x5eed1234;
  const next=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
  for(let i=0;i<5000;i++){
    const x=(next()%65536)-32768;
    const y=(next()%65536)-32768;
    const min=Math.min(x,y),max=Math.max(x,y);
    const d=descriptorFromSchema({type:'integer',minimum:min,maximum:max});
    assert.equal(d.minimum,min);assert.equal(d.maximum,max);
    assert.equal(d.kind,2);assert.deepEqual(d.excluded,[]);
    assert.throws(()=>descriptorFromSchema({type:'integer',minimum:min,
      maximum:max,exclusiveMaximum:max}),/ZORDE_STATIC_REFUSED/);
  }
});
test('5,000 randomized string intervals preserve codepoint bounds and refuse regex',()=>{
  let seed=0x7f12bead;
  const next=()=>{seed=(Math.imul(seed,1103515245)+12345)>>>0;return seed;};
  for(let i=0;i<5000;i++){
    const x=next()%65537,y=next()%65537;
    const min=Math.min(x,y),max=Math.max(x,y);
    const d=descriptorFromSchema({type:'string',minLength:min,maxLength:max});
    assert.equal(d.minimum,min);assert.equal(d.maximum,max);
    assert.equal(d.kind,3);assert.deepEqual(d.excluded,[]);
    assert.throws(()=>descriptorFromSchema({type:'string',minLength:min,
      maxLength:max,pattern:'^a$'}),/ZORDE_STATIC_REFUSED/);
  }
});

test('static pilot guest has real native assertions and refuses unexpected schema ranges',()=>{
  const {ir,report}=fixture();
  const pilot=deriveStaticRegistry(ir,report);
  const label={schemaId:'Zorde.Validation.ShortLabel',
    descriptor:{version:1,kind:3,property:2,minimum:1,maximum:64,excluded:[]}};
  pilot.entries.push(label);
  const source=emitZordePilotGuestSmoke(pilot);
  assert.match(source,/zorde_compile_scalar_registry\(rt borrow specs\)/);
  assert.match(source,/value_int\(65535\)/);
  assert.match(source,/value_int\(65536\)/);
  assert.match(source,/ShortLabel 65 characters rejected/);
  assert.match(source,/malformed UTF-8 rejected/);
  assert.match(source,/UTF-8 byte length cannot replace Unicode scalar count/);
  assert.match(source,/UTF-8 surrogate encoding rejected/);
  assert.match(source,/zorde generated registry guest smoke passed/);
  assert.equal(emitZordePilotGuestSmoke(pilot),source);
  const narrowed=structuredClone(pilot);
  narrowed.entries[0].descriptor.maximum=5;
  assert.throws(()=>emitZordePilotGuestSmoke(narrowed),/ZORDE_STATIC_REFUSED/);
  const widened=structuredClone(pilot);
  widened.entries[1].descriptor.maximum=65;
  assert.throws(()=>emitZordePilotGuestSmoke(widened),/ZORDE_STATIC_REFUSED/);
  const extra=structuredClone(pilot);
  extra.entries.push({schemaId:'Zorde.Other',
    descriptor:{version:1,kind:2,property:1,minimum:1,maximum:2,excluded:[]}});
  assert.throws(()=>emitZordePilotGuestSmoke(extra),/ZORDE_STATIC_REFUSED/);
});

const badShapes=[
  {type:'integer',minimum:1.25,maximum:3},
  {type:'integer',minimum:1,maximum:3,exclusiveMinimum:1},
  {type:'integer',minimum:1,maximum:3,multipleOf:2},
  {type:'integer',minimum:1,maximum:3,default:2},
  {type:'integer',minimum:1,maximum:3,$ref:'#/foo'},
  {type:'integer',minimum:-2147483649,maximum:3},
  {type:'integer',minimum:3,maximum:2},
  {type:'integer',minimum:1,maximum:3,format:'int64'},
  {type:'integer',minimum:1,maximum:3,anyOf:[]},
  {type:'string',minLength:0,maxLength:64,pattern:'^[a-z]+$'},
  {type:'string',minLength:0,maxLength:64,format:'email'},
  {type:'string',minLength:0,maxLength:64,const:'ok'},
  {type:'string',minLength:10,maxLength:1},
  {type:'string',minLength:0,maxLength:65537},
  {type:'string',minLength:0},
  {type:'string',maxLength:64},
  {type:'number',minimum:1,maximum:2},
];
for (const [idx,scalar] of badShapes.entries()) {
  test('unsupported scalar semantics refuse #' + (idx+1),()=>{
    assert.throws(()=>descriptorFromSchema(scalar),/ZORDE_STATIC_REFUSED/);
  });
}
test('forged runId is rejected even after rewriting IR receipt and self digest',()=>{
  const {ir,report}=fixture();
  report.runId='e'.repeat(64);
  ir.admission.receipt.runId=report.runId;
  ir.admission.receipt.digest=canonicalDigest(semanticReportProjection(report));
  const {irId:_,...body}=ir;
  ir.irId=canonicalDigest(body);
  assert.throws(()=>deriveStaticRegistry(ir,report),/runId content digest mismatch/);
});
test('changed compiler version invalidates runId even if receipt self digest is rewritten',()=>{
  const {ir,report}=fixture();
  report.toolchain.typespecCompiler.version='999.0.0';
  ir.admission.receipt.digest=canonicalDigest(semanticReportProjection(report));
  const {irId:_,...body}=ir;ir.irId=canonicalDigest(body);
  assert.throws(()=>deriveStaticRegistry(ir,report),/runId content digest mismatch/);
});
test('schema-body substitution is rejected despite forged matching IR self digest',()=>{
  const {ir,report}=fixture();
  const d=ir.declarations[0];
  d.assertionSchema.minimum=2;
  d.lanes.typespecGeneratedJsonSchema.normalizedSchema.minimum=2;
  d.lanes.authoredJsonSchema.normalizedSchema.minimum=2;
  const {irId:_,...body}=ir;
  ir.irId=canonicalDigest(body);
  assert.throws(()=>deriveStaticRegistry(ir,report),/ZORDE_STATIC_REFUSED/);
});
test('forged schema digest without recomputing IR digest fails',()=>{
  const {ir,report}=fixture();
  ir.declarations[0].lanes.authoredJsonSchema.schemaDigest=h('f');
  assert.throws(()=>deriveStaticRegistry(ir,report),/ZORDE_STATIC_REFUSED/);
});
test('receipt semantic-projection mutation cannot survive unchanged receipt digest',()=>{
  const {ir,report}=fixture();
  report.configuration.differential.maxProbesPerDeclarationPerLane=0;
  assert.throws(()=>deriveStaticRegistry(ir,report),/ZORDE_STATIC_REFUSED/);
});
test('nonsemantic host paths are intentionally excluded from semantic receipt digest',()=>{
  const {ir,report}=fixture();
  report.inputs.typespec.input='/host-dependent/src/contract.tsp';
  report.inputs.authoredJsonSchema.input='/tmp/runtime/path/schema.json';
  report.configuration.emitterOptions={'emitter-output-dir':'/tmp/generated'};
  // Different semantic configuration (emitterOptions null => {}) is significant.
  assert.throws(()=>deriveStaticRegistry(ir,report),/ZORDE_STATIC_REFUSED/);
});
test('unrecognized JSON Schema dialect is refused even with valid scalar bounds',()=>{
  assert.throws(()=>descriptorFromSchema({type:'integer',minimum:1,maximum:2,
    $schema:'https://json-schema.org/draft-07/schema#'}),/ZORDE_STATIC_REFUSED/);
});

test('canonical SHA-256 matches external fixture and rejects inconsistent schema digests',()=>{
  assert.equal(canonicalDigest({b:2,a:1}),
    '43258cff783fe7036d8a43033f830adfc60ec037382473548ac742b888292777');
  const {ir,report}=fixture();
  const d=ir.declarations[0];
  d.lanes.authoredJsonSchema.normalizedSchema.minimum=2;
  // A separately recomputed IR self digest must not replace the unchanged
  // independently authored schema lane digest.
  const {irId:_,...body}=ir;
  ir.irId=canonicalDigest(body);
  assert.throws(()=>deriveStaticRegistry(ir,report),/schema lane digest/);
});
test('input evidence reads are bounded, no-follow and cryptographically verified',()=>{
  const tmp=mkdtempSync(join(realpathSync(tmpdir()),'zorde-input-'));
  try {
    const p=join(tmp,'good.json'),bytes=Buffer.from('{"a":1}','utf8');
    writeFileSync(p,bytes);
    if(!Number.isInteger(fsConstants.O_NOFOLLOW)||fsConstants.O_NOFOLLOW===0
      ||!Number.isInteger(fsConstants.O_NONBLOCK)) {
      assert.throws(()=>readEvidenceFile(p,hash(bytes)),
        /ZORDE_STATIC_REFUSED: platform cannot guarantee/);
      return;
    }
    assert.deepEqual(readEvidenceFile(p,hash(bytes)),{a:1});
    assert.throws(()=>readEvidenceFile(p,'0'.repeat(64)),/ZORDE_STATIC_REFUSED/);
    const sym=join(tmp,'link.json');
    symlinkSync(p,sym);
    assert.throws(()=>readEvidenceFile(sym,hash(bytes)));
    const parentSymlink=join(tmp,'symlinked-evidence-parent');
    symlinkSync(tmp,parentSymlink,'dir');
    assert.throws(()=>readEvidenceFile(join(parentSymlink,'good.json'),hash(bytes)),
      /ZORDE_STATIC_REFUSED/,
      'a real input file must not bypass its symlinked parent directory');
    const other=join(tmp,'hardlink.json');
    linkSync(p,other);
    assert.throws(()=>readEvidenceFile(p,hash(bytes)),/ZORDE_STATIC_REFUSED/);
    const wide=join(tmp,'large.json');
    writeFileSync(wide,Buffer.alloc(2097153,32));
    assert.throws(()=>readEvidenceFile(wide,hash(readFileSync(wide))),/ZORDE_STATIC_REFUSED/);
    const invalid=join(tmp,'invalid.json');
    const badUtf8=Buffer.from([123,34,97,34,58,34,0xff,34,125]);
    writeFileSync(invalid,badUtf8);
    assert.throws(()=>readEvidenceFile(invalid,hash(badUtf8)));
    const dup=join(tmp,'duplicate.json'),dupBytes=Buffer.from('{"a":1,"\\u0061":2}');
    writeFileSync(dup,dupBytes);
    assert.throws(()=>readEvidenceFile(dup,hash(dupBytes)),/ZORDE_STATIC_REFUSED/);
  } finally {rmSync(tmp,{recursive:true,force:true});}
});
function attachReviewedCorpus(ir,report,root){
  const corpus=reviewedCorpusIdentity(root);
  report.configuration.differential.instanceCorpusDigest=corpus.digest;
  const projection=semanticReportProjection(report);
  report.runId=canonicalDigest({
    schema:report.schema,status:report.status,
    configuration:projection.configuration,
    inputDigests:{
      typespec:report.inputs.typespec.digest,
      authored:report.inputs.authoredJsonSchema.digest,
      generated:report.inputs.generatedJsonSchema.digest},
    toolchain:projection.toolchain,findings:[],
  });
  ir.admission.receipt.runId=report.runId;
  ir.admission.receipt.digest=canonicalDigest(semanticReportProjection(report));
  const {irId:ignored,...body}=ir;ir.irId=canonicalDigest(body);
  return corpus;
}
function writePilotCorpus(root){
  const base=join(root,'reviewed-corpus');
  mkdirSync(base,{mode:0o700});
  mkdirSync(join(base,'Port'),{mode:0o700});
  for(const lane of ['valid','invalid'])mkdirSync(join(base,'Port',lane),{mode:0o700});
  writeFileSync(join(base,'Port','valid','lower.json'),'1\n');
  writeFileSync(join(base,'Port','valid','upper.json'),'65535\n');
  writeFileSync(join(base,'Port','invalid','below.json'),'0\n');
  return base;
}
test('reviewed corpus SHA-256 is sensitive to positives, negatives, names and symlinks',()=>{
  const tmp=mkdtempSync(join(realpathSync(tmpdir()),'zorde-corpus-'));
  try{
    const root=writePilotCorpus(tmp);
    const baseline=reviewedCorpusIdentity(root);
    assert.equal(baseline.corpus.length,3);
    assert.deepEqual(baseline.corpus.map(x=>x.expectation),[
      'accepted','accepted','rejected']);
    assert.equal(baseline.digest,canonicalDigest(baseline.corpus));
    writeFileSync(join(root,'Port','invalid','below.json'),'65536\n');
    assert.notEqual(reviewedCorpusIdentity(root).digest,baseline.digest);
    symlinkSync(join(root,'Port','valid','lower.json'),
      join(root,'Port','valid','linked.json'));
    assert.throws(()=>reviewedCorpusIdentity(root),/ZORDE_STATIC_REFUSED/);
    const alt=join(tmp,'linked-root');
    symlinkSync(root,alt,'dir');
    assert.throws(()=>reviewedCorpusIdentity(alt),/ZORDE_STATIC_REFUSED/);
    const linkedParent=join(tmp,'linked-parent');
    symlinkSync(tmp,linkedParent,'dir');
    assert.throws(()=>reviewedCorpusIdentity(join(linkedParent,'reviewed-corpus')),
      /ZORDE_STATIC_REFUSED/);
  } finally{rmSync(tmp,{recursive:true,force:true});}
});

test('Node CLI emits complete write-once artifact and refuses output symlink',()=>{
  const tmp=mkdtempSync(join(realpathSync(tmpdir()),'zorde-plan-'));
  try {
    const {ir,report}=fixture();
    const corpusRoot=writePilotCorpus(tmp);
    const reviewed=attachReviewedCorpus(ir,report,corpusRoot);
    assert.equal(reviewed.corpus.length,3);
    const irFile=join(tmp,'contract-ir.json'),reportFile=join(tmp,'report.json');
    const ib=Buffer.from(JSON.stringify(ir)),rb=Buffer.from(JSON.stringify(report));
    writeFileSync(irFile,ib);writeFileSync(reportFile,rb);
    const script=resolve('scripts/zorde-static-contract-ir.mjs');
    const args=[script,'--ir='+irFile,'--report='+reportFile,
      '--ir-sha256='+hash(ib),'--report-sha256='+hash(rb),
      '--out-dir=artifacts/zorde-generated','--instances='+corpusRoot];
    const r=spawnSync(process.execPath,args,{cwd:tmp,encoding:'utf8',timeout:15000});
    assert.equal(r.status,0,r.stderr);
    const dir=join(tmp,'artifacts','zorde-generated');
    const marker=JSON.parse(readFileSync(join(dir,'build-complete.json'),'utf8'));
    const plan=readFileSync(join(dir,'registry-plan.json'));
    const guest=readFileSync(join(dir,'registry-specs.ores'));
    const smoke=readFileSync(join(dir,'registry-smoke.ores'));
    assert.equal(marker.planSha256,hash(plan));
    assert.equal(marker.guestSha256,hash(guest));
    assert.equal(marker.smokeSha256,hash(smoke));
    assert.equal(marker.reviewedCorpusDigest,reviewed.digest);
    assert.equal(marker.reviewedCorpusInstances,3);
    assert.equal(marker.guestSmokeCompiled,false);
    assert.equal(marker.guestSmokeExecuted,false);
    assert.equal(marker.nativeGuestExecuted,false);
    assert.equal(marker.releaseReady,false);
    assert.equal(spawnSync(process.execPath,args,{cwd:tmp,encoding:'utf8'}).status,2,
      'existing outputs must not be silently overwritten');
    // An unchanged report/IR plus mutated reviewed instance bytes cannot
    // create a new plan, even if all scalar declaration bodies still agree.
    rmSync(dir,{recursive:true,force:true});
    writeFileSync(join(corpusRoot,'Port','valid','lower.json'),'0\n');
    const changedCorpus=spawnSync(process.execPath,args,{
      cwd:tmp,encoding:'utf8',timeout:15000});
    assert.equal(changedCorpus.status,2,
      'stale source-bound corpus receipt must refuse changed fixtures');
    assert.match(changedCorpus.stderr,/current corpus does not match/);
    assert.equal(existsSync(dir),false,
      'no generated output may precede the corpus identity check');

    const tmp2=mkdtempSync(join(realpathSync(tmpdir()),'zorde-plan-link-'));
    try{
      mkdirSync(join(tmp2,'elsewhere'));
      symlinkSync(join(tmp2,'elsewhere'),join(tmp2,'artifacts'),'dir');
      const denied=spawnSync(process.execPath,args,{cwd:tmp2,encoding:'utf8'});
      assert.equal(denied.status,2);
    } finally {rmSync(tmp2,{recursive:true,force:true});}
  } finally {rmSync(tmp,{recursive:true,force:true});}
});

const corruption=[
  ['status',ir=>{ir.admissible=false;}],
  ['missing fixture digest',(_,r)=>{delete r.configuration.differential.instanceCorpusDigest;}],
  ['differential corpus identity differs from its configuration',(_,r)=>{
    r.differential.corpusDigest=h('7');}],
  ['probe-only corpus',(_,r)=>{r.differential.summary.corpusInstances=0;}],
  ['unmatched reviewed corpus',(_,r)=>{r.differential.summary.corpusInstances=4;}],
  ['summary falsely claims partial fixture coverage',(_,r)=>{
    r.differential.summary.declarationsWithReviewedPositiveAndNegative=0;}],
  ['declaration lacks negative fixtures',(_,r)=>{
    r.differential.declarations[0].reviewedCorpus.rejected=0;}],
  ['positive-only incorrectly claimed as reviewed',(_,r)=>{
    r.differential.declarations[0].reviewedFixtureCoverage='positive-only';}],
  ['per-declaration counts forged',(_,r)=>{
    r.differential.declarations[0].reviewedCorpus.total=4;}],
  ['duplicate reviewed declaration',(_,r)=>{
    r.differential.declarations.push(structuredClone(r.differential.declarations[0]));}],
  ['reviewed corpus nominal declaration swapped',(_,r)=>{
    r.differential.declarations[0].typespec='Zorde.Validation.Evil';}],
  ['per-declaration authored name spoofed',(_,r)=>{
    r.differential.declarations[0].authored='Different';}],
  ['unreviewed corpus instance tolerated',(_,r)=>{
    r.differential.summary.reviewedUnassertedInstances=1;}],
  ['reviewed instance mismatch with aggregated summary',(_,r)=>{
    r.differential.summary.reviewedRejectedInstances=7;}],
  ['nonzero counts findings',(_,r)=>{r.counts.findings=1;}],
  ['truncated finding evidence',(_,r)=>{r.counts.findingsTruncated=true;}],
  ['nonzero structural finding inventory',(_,r)=>{r.counts.structuralFindings=1;}],
  ['nonzero differential finding inventory',(_,r)=>{r.counts.differentialFindings=1;}],
  ['disabled source mutation check',(_,r)=>{r.coverage.sourceMutationCheck=false;}],
  ['compare not full source check',(_,r)=>{r.configuration.mode='compare';}],
  ['wrong mapped kind',(_,r)=>{r.declarationMap[0].kind='model';}],
  ['role',ir=>{ir.role='editable-contract';}],
  ['peer authority',ir=>{ir.authorities.precedence='typespec';}],
  ['excluded declaration',ir=>{ir.excludedDeclarations.push({id:'x'});}],
  ['excluded scope',ir=>{ir.admission.scope.complete=false;}],
  ['unknown kind',ir=>{ir.declarations[0].kind='model';}],
  ['duplicate declaration',ir=>{ir.declarations.push(structuredClone(ir.declarations[0]));}],
  ['invalid ID',ir=>{ir.declarations[0].id='Bad"); panic();';}],
  ['attacker-added narrowing keyword',ir=>{
    ir.declarations[0].lanes.authoredJsonSchema.normalizedSchema.const=5;}],
  ['different interval in authored schema',ir=>{
    ir.declarations[0].lanes.authoredJsonSchema.normalizedSchema.minimum=2;}],
  ['different interval in generated schema',ir=>{
    ir.declarations[0].lanes.typespecGeneratedJsonSchema.normalizedSchema.maximum=9;}],
  ['claim of no generated authority',ir=>{
    ir.declarations[0].lanes.typespecGeneratedJsonSchema.role='independently-authored-authority';}],
  ['changed input digest',ir=>{ir.provenance.typespec.digest=h('f');}],
  ['disabled differential',(_,r)=>{r.coverage.differentialInstanceValidation=false;}],
  ['refused differential',(_,r)=>{r.differential.summary.refusals=1;}],
  ['no probes',(_,r)=>{r.differential.summary.probesEvaluated=0;}],
  ['nonempty findings',(_,r)=>{r.findings.push({ruleId:'drift'});}],
  ['changed report run identity',(_,r)=>{r.runId=h('0');}],
  ['unbound declaration mapping',(_,r)=>{r.declarationMap[0].generated='Renamed';}],
  ['not reviewed coverage',ir=>{ir.admission.requirements.exactInputDigests=false;}],
];
for(const [name,change] of corruption) {
  test('refuses ' + name,()=>refuse(change));
}
