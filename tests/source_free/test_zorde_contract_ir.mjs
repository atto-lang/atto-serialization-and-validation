import test from 'node:test';
import assert from 'node:assert/strict';
import {deriveStaticRegistry, descriptorFromSchema, emitOreslangSpecs, rejectDuplicateJsonKeys} from
  '../../scripts/zorde-static-contract-ir.mjs';

const h = (c) => c.repeat(64);
function fixture() {
  const report = {
    schema:'ores.typespec-json-schema-validator.report/v1',
    runId:h('a'),status:'passed',zeroUnexplainedFindings:true,findings:[],
    coverage:{directDeclarationInventory:true,
      typespecGeneratedJsonSchemaComparison:true,differentialInstanceValidation:true},
    differential:{summary:{probesEvaluated:22,divergences:0,refusals:0}},
    inputs:{typespec:{digest:h('b')},authoredJsonSchema:{digest:h('c')},
      generatedJsonSchema:{digest:h('d')}},
    declarationMap:[{typespec:'Zorde.Validation.Port',generated:'Port',authored:'Port'}],
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
const corruption=[
  ['status',ir=>{ir.admissible=false;}],
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
