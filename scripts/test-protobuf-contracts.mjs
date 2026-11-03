#!/usr/bin/env node
// Conservative contract drift tripwire: NOT TypeSpec compiler evidence,
// JSON Schema validator execution, wasm-tools parsing, or a WIT ABI proof.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

const tsp=readFileSync('contracts/protobuf/main.tsp','utf8');
const wit=readFileSync('wit/protobuf-wire.wit','utf8');
const schema=JSON.parse(readFileSync('contracts/protobuf/authored.schema.json','utf8'));
function canonical(s) {
  return s.replace(/\/\/[^\n]*/g,'').replace(/\s+/g,' ').trim();
}
function exactKeys(obj,keys) {
  assert.ok(obj && typeof obj==='object' && !Array.isArray(obj));
  assert.deepEqual(Object.keys(obj).sort(),[...keys].sort());
}
function audit(t,w,j) {
  assert.equal(canonical(t),
    canonical(`
import "@typespec/json-schema";
using TypeSpec.JsonSchema;
namespace Oreslang.Protobuf;
@id("WireField")
model WireField {
  @minValue(1)
  @maxValue(536870911)
  fieldNumber: int32;
  wireType: 0 | 1 | 2 | 5;
  payload: uint8[];
}
@id("WireMessage")
model WireMessage {
  fields: WireField[];
}`),'TypeSpec independent authority changed');

  assert.equal(canonical(w),
    canonical(`
package oreslang:protobuf-wire@0.1.0;
interface protobuf-wire {
  record wire-field {
    field-number: u32,
    wire-type: u8,
    payload: list<u8>,
  }
  record wire-message {
    fields: list<wire-field>,
  }
  record decode-limits {
    max-input-bytes: u32,
    max-fields: u32,
    max-length-delimited-bytes: u32,
  }
  record encode-limits {
    max-output-bytes: u32,
  }
  record codec-error {
    code: string,
    offset: u32,
  }
  decode: func(data: list<u8>, limits: decode-limits) -> result<wire-message, codec-error>;
  encode: func(message: wire-message, limits: encode-limits) -> result<list<u8>, codec-error>;
}
world protobuf-wire-guest {
  export protobuf-wire;
}`),'WIT ABI proposal changed');

  exactKeys(j,['$schema','$id','$defs']);
  assert.equal(j.$schema,'https://json-schema.org/draft/2020-12/schema');
  assert.equal(j.$id,'https://oreslang.dev/schemas/protobuf-wire-envelope-v1.schema.json');
  exactKeys(j.$defs,['WireField','WireMessage']);
  const f=j.$defs.WireField,m=j.$defs.WireMessage;
  for(const [o,id] of [[f,'WireField'],[m,'WireMessage']]) {
    assert.equal(o.$id,id);
    assert.equal(o.$schema,j.$schema);
    assert.equal(o.type,'object');
    assert.equal(o.unevaluatedProperties,false);
    exactKeys(o,['$schema','$id','type','properties','required','unevaluatedProperties']);
  }
  exactKeys(f.properties,['fieldNumber','wireType','payload']);
  assert.deepEqual(f.required,['fieldNumber','wireType','payload']);
  assert.deepEqual(f.properties.fieldNumber,{type:'integer',minimum:1,maximum:536870911});
  assert.deepEqual(f.properties.wireType,{type:'integer',enum:[0,1,2,5]});
  assert.deepEqual(f.properties.payload,{
    type:'array',items:{type:'integer',minimum:0,maximum:255}
  });
  exactKeys(m.properties,['fields']);
  assert.deepEqual(m.required,['fields']);
  assert.deepEqual(m.properties.fields,{type:'array',items:{$ref:'WireField'}});
}
audit(tsp,wit,schema);
let checks=1;
function reject(fn) { assert.throws(fn); checks++; }
const mutant=(mutate)=>{
  const j=structuredClone(schema);
  mutate(j);
  return ()=>audit(tsp,wit,j);
};
reject(mutant(j=>{j.$defs.WireField.properties.fieldNumber.minimum=0;}));
reject(mutant(j=>{j.$defs.WireField.properties.fieldNumber.maximum=536870912;}));
reject(mutant(j=>{j.$defs.WireField.properties.wireType.enum.push(3);}));
reject(mutant(j=>{j.$defs.WireField.properties.payload.items.maximum=256;}));
reject(mutant(j=>{j.$defs.WireField.properties.payload.uniqueItems=true;}));
reject(mutant(j=>{delete j.$defs.WireField.unevaluatedProperties;}));
reject(mutant(j=>{j.$defs.WireMessage.properties.secret={type:'string'};}));
reject(mutant(j=>{j.$defs.WireMessage.properties.fields.items.$ref='Secret';}));
reject(mutant(j=>{j.$defs.WireField.required.pop();}));
reject(mutant(j=>{j.$defs.WireMessage.$id='WireMessageV2';}));
reject(()=>audit(tsp.replace('wireType: 0 | 1 | 2 | 5','wireType: int32'),wit,schema));
reject(()=>audit(tsp.replace('@minValue(1)','@minValue(0)'),wit,schema));
reject(()=>audit(tsp,wit.replace('payload: list<u8>','payload: string'),schema));
reject(()=>audit(tsp,wit.replace('export protobuf-wire','import protobuf-wire'),schema));
reject(()=>audit(tsp,wit+'\nworld unexpected {}',schema));

// The source audit is a narrow lexical tripwire, NOT compilation or an
// executable validator proof. Require the reviewed complete-wire budget path
// and forbid the retired modulo operator in this Protobuf .ores implementation.
const wire=readFileSync('src/protobuf/wire.ores','utf8');
const zorde=readFileSync('src/protobuf/zorde.ores','utf8');
function sourceGuard(w,z) {
  assert.ok(!w.includes('%'), 'unsupported modulo operator in native wire codec');
  assert.ok(w.includes('pub fnc proto_field_reencoded_wire_bytes('),
    'missing full encoded field size API');
  assert.ok(w.includes('proto_unsigned_varint_size(value_bytes)'),
    'missing length-prefix byte accounting');
  assert.ok(z.includes('proto_field_reencoded_wire_bytes(field)'),
    'Zorde fails to count field keys and length prefixes');
  assert.ok(z.includes('options.max_input_bytes - total_reencoded_wire_bytes'),
    'Zorde lacks overflow-safe cumulative budget');
  assert.ok(z.includes('if field.payload.length >= remaining_bytes then'),
    'missing cheap payload budget preflight');
  assert.ok(z.indexOf('if field.payload.length >= remaining_bytes then') <
    z.indexOf('proto_field_reencoded_wire_bytes(field)'),
    'payload budget preflight occurs after shape scan');
  const copyStart=w.indexOf('pub fnc proto_message_copy_bounded(');
  assert.ok(copyStart>=0,'missing bounded Protobuf message copy helper');
  const copyBudget=w.indexOf('if field.payload.length >= remaining_bytes then',copyStart);
  const copyDetached=w.indexOf('val ProtoField detached = proto_field_snapshot(field);',copyStart);
  const copyDetachedBudget=w.indexOf('if detached.payload.length >= remaining_bytes then',copyStart);
  const copyScan=w.indexOf('proto_field_reencoded_wire_bytes(detached)',copyStart);
  const copyAppend=w.indexOf('copy.add(detached);',copyStart);
  assert.ok(copyBudget>copyStart && copyDetached>copyBudget &&
    copyDetachedBudget>copyDetached && copyScan>copyDetachedBudget &&
    copyAppend>copyScan,
    'bounded copy must validate detached bytes after cheap source preflight');
  const cappedExit='if report.truncated then\n      return report;\n    fi';
  const fieldExit=z.indexOf(cappedExit);
  const requiredExit=z.indexOf(cappedExit,fieldExit+1);
  assert.ok(fieldExit>z.indexOf('val int payload_bytes = proto_field_payload_wire_bytes(field)') &&
    fieldExit<z.indexOf('field_index = field_index + 1;'),
    'Zorde continues scanning message.fields after report truncation');
  assert.ok(requiredExit>z.indexOf('if rules[i].required && occurrences[i] == 0 then') &&
    requiredExit<z.lastIndexOf('i = i + 1;'),
    'Zorde continues scanning required rules after report truncation');
}
sourceGuard(wire,zorde);
checks++;

// Source-only defense-in-depth: both actual encoder and report-only admission
// must enforce the same raw-varint/signed-metadata consistency constraint.
// Mutation probes ensure this tripwire catches removing either enforcement.
const protobufWireTests=readFileSync('tests/protobuf_test.ores','utf8');
const protobufZordeTests=readFileSync('tests/protobuf_zorde_test.ores','utf8');
function varintMetadataGuard(w,t,zt) {
  assert.ok(w.includes('fnc raw_varint_matches_declared_int(ProtoField field): bool'),
    'missing declared integer/raw Protobuf consistency helper');
  assert.ok(w.includes('if field.payload.length > 9 then') ||
    w.includes('field.payload.length > 9 then'),
    'signed raw varints longer than nine bytes must be denied');
  assert.ok(w.includes('return decoded == field.varint_value;'),
    'raw varint decoder must compare with declared signed metadata');
  assert.ok(w.includes('return raw_varint_is_valid(field.payload) &&\n             raw_varint_matches_declared_int(field);'),
    'report-only field admission must enforce metadata consistency');
  assert.ok(w.includes('!raw_varint_matches_declared_int(field) then'),
    'encoder must refuse inconsistent varint metadata');
  assert.ok(t.includes('val ProtoField forged_integer =') &&
    t.includes('val ProtoField consistent_nonminimal =') &&
    t.includes('val ProtoField misclaimed_ten_byte ='),
    'native wire tests must cover forged, legitimate and ten-byte cases');
  assert.ok(zt.includes('forged_metadata_report.errors[0].code == "wrong-type"') &&
    zt.includes('matching_nonminimal_report.errors.length, 0') &&
    zt.includes('false_signed_fit_report.errors[0].code == "wrong-type"'),
    'native Zorde tests must cover forged and legitimate raw varints');
}
varintMetadataGuard(wire,protobufWireTests,protobufZordeTests);
checks++;
reject(()=>varintMetadataGuard(wire.replace(
  'return decoded == field.varint_value;', 'return true;'),
  protobufWireTests,protobufZordeTests));
reject(()=>varintMetadataGuard(wire.replace(
  'raw_varint_is_valid(field.payload) &&\n             raw_varint_matches_declared_int(field);',
  'raw_varint_is_valid(field.payload);'),
  protobufWireTests,protobufZordeTests));
reject(()=>varintMetadataGuard(wire.replace(
  '!raw_varint_matches_declared_int(field) then', 'false then'),
  protobufWireTests,protobufZordeTests));
reject(()=>varintMetadataGuard(wire,protobufWireTests.replace(
  'val ProtoField forged_integer =', 'val ProtoField untested_forged_integer ='),
  protobufZordeTests));
reject(()=>varintMetadataGuard(wire,protobufWireTests,
  protobufZordeTests.replace('forged_metadata_report.errors[0].code == "wrong-type"',
    'forged_metadata_report.errors[0].code == "accepted"')));
// Opaque raw and non-varint fields have zero-sentinel integer metadata.
// These are lexical/mutation checks only: NOT native guest or CI proof.
function opaqueMetadataGuard(w,t,zt) {
  assert.ok(w.includes('fnc proto_field_metadata_shape_valid(ProtoField field): bool'),
    'missing field-wide signed metadata canonicalization');
  assert.ok(w.includes('return field.varint_fits_int || field.varint_value == 0;') &&
    w.includes('return !field.varint_fits_int && field.varint_value == 0;'),
    'opaque and non-varint fields must use canonical zero metadata');
  assert.ok(w.includes('!valid_field_number(field.field_number) ||\n     !proto_field_metadata_shape_valid(field) then'),
    'report-only validator must refuse forged non-varint metadata');
  assert.ok(w.includes('if !proto_field_metadata_shape_valid(field) then'),
    'actual encoder must refuse forged non-varint metadata');
  assert.ok(w.includes('return new VarintRead(true, false, 0, bytes);'),
    'raw-only decoder must not expose partially decoded signed integer');
  assert.ok(t.includes('val ProtoField opaque_forged_integer =') &&
    t.includes('val ProtoField forged_delimited_metadata =') &&
    t.includes('val ProtoField forged_fixed_metadata =') &&
    t.includes('raw_roundtrip.fields[0].varint_value, 0'),
    'wire guests must cover opaque/fixed/delimited and decode sentinel');
  assert.ok(zt.includes('val List<ProtoField> forged_opaque_metadata =') &&
    zt.includes('val List<ProtoField> forged_nonvarint_metadata ='),
    'Zorde guests must refuse field-wide forged signed metadata');
}
opaqueMetadataGuard(wire,protobufWireTests,protobufZordeTests);
checks++;
reject(()=>opaqueMetadataGuard(wire.replace(
  'return !field.varint_fits_int && field.varint_value == 0;',
  'return true;'),protobufWireTests,protobufZordeTests));
reject(()=>opaqueMetadataGuard(wire.replace(
  '!valid_field_number(field.field_number) ||\n     !proto_field_metadata_shape_valid(field) then',
  '!valid_field_number(field.field_number) then'),
  protobufWireTests,protobufZordeTests));
reject(()=>opaqueMetadataGuard(wire.replace(
  'if !proto_field_metadata_shape_valid(field) then',
  'if false then'),protobufWireTests,protobufZordeTests));
reject(()=>opaqueMetadataGuard(wire.replace(
  'return new VarintRead(true, false, 0, bytes);',
  'return new VarintRead(true, false, value, bytes);'),
  protobufWireTests,protobufZordeTests));
reject(()=>opaqueMetadataGuard(wire,
  protobufWireTests.replace('val ProtoField forged_fixed_metadata =',
    'val ProtoField untested_fixed_metadata ='),
  protobufZordeTests));
reject(()=>opaqueMetadataGuard(wire,protobufWireTests,
  protobufZordeTests.replace('val List<ProtoField> forged_opaque_metadata =',
    'val List<ProtoField> untested_opaque_metadata =')));

// Independent source-only mutation tests for checked Protobuf field lookup.
// Public unbounded lookup remains for compatibility with already trusted
// inputs, but must not be used as an untrusted admission surface.
function boundedFieldLookupGuard(w,t) {
  const start=w.indexOf('pub fnc fields_with_number_bounded(');
  const legacy=w.indexOf('pub fnc fields_with_number(');
  assert.ok(start>legacy && legacy>=0,
    'checked lookup must be separately available alongside legacy API');
  const checked=w.slice(start);
  const preflight=checked.indexOf('proto_message_copy_bounded(message, options)');
  const fail=checked.indexOf('if detached.is_err() then');
  const selection=checked.indexOf('for const field of detached.unwrap().fields do');
  assert.ok(preflight>=0 && fail>preflight && selection>fail,
    'checked lookup must validate and snapshot WHOLE message before filtering');
  assert.ok(checked.includes('if !valid_field_number(field_number) then'),
    'bounded lookup must reject invalid field numbers');
  assert.ok(checked.includes('return Ok(selected);'),
    'bounded lookup must return detached selected fields');
  for(const marker of [
    'bounded_lookup.is_err()',
    'lookup_too_small.is_err()',
    'lookup_too_many.is_err()',
    'lookup_invalid_number.is_err()',
    'lookup_invalid_other.is_err()',
    'bounded_two[0].payload.add(90);',
    'roundtrip.fields[1].payload.length, 2'
  ]) {
    assert.ok(t.includes(marker), 'native bounded lookup test missing: '+marker);
  }
}
boundedFieldLookupGuard(wire,protobufWireTests);
checks++;
reject(()=>boundedFieldLookupGuard(wire.replace(
  'proto_message_copy_bounded(message, options)', 'Ok(message)'), protobufWireTests));
reject(()=>boundedFieldLookupGuard(wire.replace(
  'for const field of detached.unwrap().fields do',
  'for const field of message.fields do'), protobufWireTests));
reject(()=>boundedFieldLookupGuard(wire.replace(
  'proto_message_copy_bounded(message, options);\n  if detached.is_err() then',
  'proto_message_copy_bounded(message, options);\n  if false then'),
  protobufWireTests));
reject(()=>boundedFieldLookupGuard(wire,protobufWireTests.replace(
  'lookup_invalid_other.is_err()', 'lookup_invalid_other.is_ok()')));
reject(()=>boundedFieldLookupGuard(wire,protobufWireTests.replace(
  'bounded_two[0].payload.add(90);',
  'roundtrip.fields[1].payload.add(90);')));

reject(()=>sourceGuard(wire+'%',zorde));
reject(()=>sourceGuard(wire.replace('pub fnc proto_field_reencoded_wire_bytes(',
  'fnc removed_full_wire_size('),zorde));
reject(()=>sourceGuard(wire.replace('proto_unsigned_varint_size(value_bytes)',
  '0'),zorde));
reject(()=>sourceGuard(wire,zorde.replace('proto_field_reencoded_wire_bytes(field)',
  'proto_field_payload_wire_bytes(field)')));
reject(()=>sourceGuard(wire,zorde.replace('options.max_input_bytes - total_reencoded_wire_bytes',
  'options.max_input_bytes')));
reject(()=>sourceGuard(wire,zorde.replace('if field.payload.length >= remaining_bytes then',
  'if false then')));
reject(()=>sourceGuard(wire.replace('copy.add(detached);',
  'copy.add(field);'),zorde));
reject(()=>sourceGuard(wire.replace('proto_field_reencoded_wire_bytes(detached)',
  'proto_field_reencoded_wire_bytes(field)'),zorde));
reject(()=>sourceGuard(wire.replace('if detached.payload.length >= remaining_bytes then',
  'if false then'),zorde));
reject(()=>sourceGuard(wire.replace('if field.payload.length >= remaining_bytes then',
  'if false then'),zorde));
// Restoring the old > bound defeats the O(1) no-space-for-tag rejection.
reject(()=>sourceGuard(wire,zorde.replace('if field.payload.length >= remaining_bytes then',
  'if field.payload.length > remaining_bytes then')));
reject(()=>sourceGuard(wire.replace('if field.payload.length >= remaining_bytes then',
  'if field.payload.length > remaining_bytes then'),zorde));
const capGuard='    if report.truncated then\n      return report;\n    fi';
const skippedCapGuard='    if false then\n      return report;\n    fi';
reject(()=>sourceGuard(wire,zorde.replace(capGuard,skippedCapGuard)));
const secondCap=zorde.lastIndexOf(capGuard);
reject(()=>sourceGuard(wire,
  zorde.slice(0,secondCap)+zorde.slice(secondCap).replace(capGuard,skippedCapGuard)));

// Source-only native admission wiring guard. This proves that the focused
// tests/guest/harness *source* is connected; it never executes a compiler or
// represents a toolchain-produced guest receipt.
const focused=readFileSync('tests/protobuf_main.ores','utf8');
const integrated=readFileSync('tests/main.ores','utf8');
const nativeHarness=readFileSync('scripts/test-zorde-registry-source-local.sh','utf8');
const marker='protobuf-wire-zorde-native-tests-passed';
function nativeGateGuard(f,i,h) {
  for(const source of [f,i]) {
    const wireIndex=source.indexOf('  run_protobuf_tests();');
    const zordeIndex=source.indexOf('  run_protobuf_zorde_tests();');
    const markerIndex=source.indexOf('stdio.stdout.println("'+marker+'");');
    assert.ok(wireIndex>=0 && zordeIndex>wireIndex && markerIndex>zordeIndex,
      'native guest marker must follow both Protobuf test entrypoints');
  }
  assert.ok(h.includes('"$ORESLANG_BIN" "$root/tests/protobuf_main.ores"'),
    'source-local harness must execute focused guest with approved compiler');
  assert.ok(h.includes('grep -Fqx "'+marker+'" "$protobuf_log"'),
    'source-local harness must check focused guest output');
  assert.ok(h.includes('grep -Fqx "'+marker+'" "$log"'),
    'full suite must independently check integrated Protobuf output');
  assert.ok(h.includes('"protobuf_focused_native_log_sha256"'),
    'native receipt must retain focused log digest');
  assert.ok(h.includes('"protobuf_focused_native_guest_executed": True'),
    'native receipt must distinguish focused guest evidence');
}
nativeGateGuard(focused,integrated,nativeHarness);
checks++;
reject(()=>nativeGateGuard(focused.replace('  run_protobuf_tests();','  skip_protobuf_tests();'),
  integrated,nativeHarness));
reject(()=>nativeGateGuard(focused,integrated.replace('  run_protobuf_zorde_tests();',
  '  skip_protobuf_zorde_tests();'),nativeHarness));
reject(()=>nativeGateGuard(focused,integrated,nativeHarness.replace(
  '"$ORESLANG_BIN" "$root/tests/protobuf_main.ores"','"$ORESLANG_BIN" "$root/tests/other.ores"')));
reject(()=>nativeGateGuard(focused,integrated,nativeHarness.replace(
  'grep -Fqx "'+marker+'" "$protobuf_log"','true')));
reject(()=>nativeGateGuard(focused,integrated,nativeHarness.replace(
  '"protobuf_focused_native_log_sha256"','"missing_hash"')));
reject(()=>nativeGateGuard(focused,integrated,nativeHarness.replace(
  '"protobuf_focused_native_guest_executed": True','"protobuf_focused_native_guest_executed": False')));
// The protected native job must reach the source-local attester through a
// clean environment. This is a source-only CI configuration guard; it does
// NOT attest that an administrator approved the runner or that it executed.
const nativeWorkflow=readFileSync('.github/workflows/runner.yml','utf8');
function workflowIsolationGuard(w) {
  const start=w.indexOf('      - name: Run actual compiler and privacy diagnostics');
  assert.ok(start>=0,'protected native workflow step missing');
  const native=w.slice(start);
  assert.ok(native.includes('env -i \\'),'native harness must strip inherited env');
  assert.ok(native.includes('PATH=/usr/bin:/bin HOME=/nonexistent LC_ALL=C TZ=UTC'),
    'native harness must bind a minimal tool and home environment');
  assert.ok(native.includes('GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null'),
    'native harness must disallow external Git configuration');
  assert.ok(native.includes('PYTHONDONTWRITEBYTECODE=1'),
    'Python guest verification must not create ignored output in checkout');
  assert.ok(native.includes('EXPECTED_SOURCE_SHA="$EXPECTED_SOURCE_SHA"'),
    'native launcher must receive approved exact commit');
  assert.ok(native.includes('EXPECTED_SOURCE_TREE="$EXPECTED_SOURCE_TREE"'),
    'native launcher must receive approved exact root tree');
  assert.ok(native.includes('EXPECTED_COMPILER_SHA256="$EXPECTED_COMPILER_SHA256"'),
    'native launcher must receive approved compiler digest');
  assert.ok(native.includes('ORESLANG_BIN="$ORESLANG_BIN"'),
    'native launcher must receive approved compiler path');
  assert.ok(native.includes('ZORDE_EVIDENCE_DIR="$ZORDE_EVIDENCE_DIR"'),
    'native launcher must retain private evidence path');
  assert.ok(native.includes('/usr/bin/bash --noprofile --norc \\'),
    'native launcher must avoid Bash startup profile resolution');
  assert.ok(!native.includes('          bash scripts/test-zorde-registry-source-local.sh'),
    'inheriting caller environment into private native attester is forbidden');
}
workflowIsolationGuard(nativeWorkflow);
checks++;
function alterProtectedNativeStep(source, search, replaceWith) {
  const start=source.indexOf('      - name: Run actual compiler and privacy diagnostics');
  assert.ok(start>=0,'protected native execution step missing');
  const native=source.slice(start);
  assert.ok(native.includes(search),'native test mutant anchor absent');
  return source.slice(0,start)+native.replace(search,replaceWith);
}
reject(()=>workflowIsolationGuard(alterProtectedNativeStep(nativeWorkflow,
  'env -i \\','env \\')));
reject(()=>workflowIsolationGuard(alterProtectedNativeStep(nativeWorkflow,
  'GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null',
  'GIT_CONFIG_NOSYSTEM=0 GIT_CONFIG_GLOBAL=~/.gitconfig')));
reject(()=>workflowIsolationGuard(nativeWorkflow.replace(
  '/usr/bin/bash --noprofile --norc \\', '/usr/bin/bash \\')));
reject(()=>workflowIsolationGuard(alterProtectedNativeStep(nativeWorkflow,
  'EXPECTED_SOURCE_TREE="$EXPECTED_SOURCE_TREE"',
  'EXPECTED_SOURCE_TREE="$GITHUB_SHA"')));

// A private, trusted self-hosted worker must not execute checkout-provided
// test scripts before it proves the exact admin-approved physical source tree.
// This is a narrow workflow source mutation guard, NOT runner evidence.
function preexecutionAttestationGuard(w) {
  const nativeJob=w.indexOf('  zorde-native-source-local:');
  const snapshot=w.indexOf('      - name: Reject unapproved source snapshot before checkout',nativeJob);
  const checkout=w.indexOf('      - uses: actions/checkout@',snapshot);
  const physical=w.indexOf('      - name: Verify exact physical checkout before source execution',checkout);
  const scripts=w.indexOf('      - name: Verify shell syntax and source-free refusal contracts',physical);
  const execute=w.indexOf('      - name: Run actual compiler and privacy diagnostics',scripts);
  assert.ok(nativeJob>=0 && snapshot>nativeJob && checkout>snapshot &&
    physical>checkout && scripts>physical && execute>scripts,
    'physical checkout admission must precede any source-local script invocation');
  const verify=w.slice(physical,scripts);
  assert.ok(verify.includes('test "$GITHUB_SHA" = "$EXPECTED_SOURCE_SHA"'),
    'unreviewed dispatch commit must reject');
  assert.ok(verify.includes('git rev-parse \'HEAD^{tree}\''),
    'root tree identity must reject stale branch');
  assert.ok(verify.includes('git show HEAD:scripts/verify-zorde-checkout.py |'),
    'physical verifier must match source-approved blob');
  assert.ok(verify.includes('cmp - scripts/verify-zorde-checkout.py'),
    'physical verifier must be byte-compared before executing');
  assert.ok(verify.includes('env -i PATH=/usr/bin:/bin'),
    'checkout attestation must scrub inherited environment');
  assert.ok(verify.includes('/usr/bin/python3 -I -S scripts/verify-zorde-checkout.py'),
    'physical Git blob verifier must run before source checks');
}
preexecutionAttestationGuard(nativeWorkflow);
checks++;
reject(()=>preexecutionAttestationGuard(nativeWorkflow.replace(
  '      - name: Verify exact physical checkout before source execution',
  '      - name: Removed source attestation')));
reject(()=>preexecutionAttestationGuard(nativeWorkflow.replace(
  'test "$GITHUB_SHA" = "$EXPECTED_SOURCE_SHA"',
  'test -n "$GITHUB_SHA"')));
reject(()=>preexecutionAttestationGuard(nativeWorkflow.replace(
  'git show HEAD:scripts/verify-zorde-checkout.py |',
  'git show HEAD:scripts/other.py |')));
reject(()=>preexecutionAttestationGuard(nativeWorkflow.replace(
  'cmp - scripts/verify-zorde-checkout.py','cat > /dev/null')));
reject(()=>preexecutionAttestationGuard(nativeWorkflow.replace(
  'env -i PATH=/usr/bin:/bin HOME=/nonexistent LC_ALL=C TZ=UTC',
  'env PATH=/usr/bin:/bin HOME=/nonexistent LC_ALL=C TZ=UTC')));

// Source-free integrity guard for the first source-execution boundary. Both
// ignored AND ordinary untracked files must be rejected before scripts are
// run, not merely by the later native harness status check.
const physicalVerifier=readFileSync('scripts/verify-zorde-checkout.py','utf8');
const refusalFixtures=readFileSync('scripts/test-zorde-source-local-contracts.sh','utf8');
function earlySourceIntegrityGuard(v,t) {
  assert.ok(v.includes('ignored = git_bytes("ls-files", "--others", "--ignored",'),
    'pre-execution physical verifier must reject ignored files');
  assert.ok(v.includes('untracked = git_bytes("ls-files", "--others", "--exclude-standard", "-z")'),
    'pre-execution physical verifier must enumerate ordinary untracked files');
  assert.ok(v.includes('if untracked:') && v.includes('fail("untracked files exist in source checkout")'),
    'physical verifier must refuse ordinary untracked files');
  assert.ok(t.includes('injected-untracked.ores') && t.includes('unreviewed guest source bytes'),
    'source-free test must create an untracked source injection');
  assert.ok(t.includes("grep -Fq 'untracked files exist in source checkout'"),
    'source-free fixture must assert targeted physical attestation denial');
}
earlySourceIntegrityGuard(physicalVerifier,refusalFixtures);
checks++;
reject(()=>earlySourceIntegrityGuard(physicalVerifier.replace(
  'untracked = git_bytes("ls-files", "--others", "--exclude-standard", "-z")',
  'untracked = b""'),refusalFixtures));
reject(()=>earlySourceIntegrityGuard(physicalVerifier.replace(
  'fail("untracked files exist in source checkout")',
  'return'),refusalFixtures));
reject(()=>earlySourceIntegrityGuard(physicalVerifier,
  refusalFixtures.replace("grep -Fq 'untracked files exist in source checkout'",
    "grep -Fq 'any diagnostic'")));
// A SHA-consistent Git symlink attests a link pathname, not the source bytes
// loaded through that link. Source-local native admission must refuse the
// tracked symlink mode, with an independent disposable Git negative fixture.
function trackedSymlinkGuard(verifier, fixtures) {
  const forbidden='tracked symlinks are forbidden in source-local native evidence';
  assert.ok(verifier.includes('if mode == b"120000":'),
    'tracked symlink mode must be refused at Git tree enumeration');
  assert.ok(verifier.includes('if expected_mode == b"120000":'),
    'blob verifier must refuse symlink mode even if directly invoked');
  assert.ok(verifier.split(forbidden).length >= 3,
    'both physical attestation layers must refuse tracked symlinks');
  assert.ok(fixtures.includes('git -C "$linked_src" add .') &&
    fixtures.includes('ln -s ../external-unreviewed-guest "$linked_src/escape.ores"'),
    'source-free fixture must commit an external-target symlink');
  assert.ok(fixtures.includes("grep -Fq '"+forbidden+"'"),
    'negative fixture must require exact symlink denial');
}
trackedSymlinkGuard(physicalVerifier,refusalFixtures);
checks++;
reject(()=>trackedSymlinkGuard(physicalVerifier.replace(
  'if mode == b"120000":','if mode == b"000000":'),refusalFixtures));
reject(()=>trackedSymlinkGuard(physicalVerifier.replace(
  'if expected_mode == b"120000":','if expected_mode == b"000000":'),refusalFixtures));
reject(()=>trackedSymlinkGuard(physicalVerifier,
  refusalFixtures.replace('ln -s ../external-unreviewed-guest "$linked_src/escape.ores"',
    'printf "safe" > "$linked_src/escape.ores"')));
// The ORIGINAL-WIRE report entrypoint must decode bounded input before using
// field-only Zorde rules. Canonicalized keys/length prefixes can otherwise
// undercount attacker-controlled bytes. Source-only wiring checks, not a
// substitute for an Oreslang compiler or guest assertion.
const zordeTests=readFileSync('tests/protobuf_zorde_test.ores','utf8');
function originalWireAdmissionGuard(z,t) {
  assert.ok(z.includes('pub fnc zorde_validate_protobuf_input('),
    'bounded original wire report entrypoint missing');
  assert.ok(z.includes('decode_message(input, options)'),
    'original wire input must be decoded under caller limits');
  assert.ok(z.includes('if decoded.is_err() then'),
    'raw wire failure must stop before Zorde field admission');
  assert.ok(z.includes('proto_zorde_issue(rejected, "wire-decode"'),
    'raw wire refusal needs a distinct fail-closed report code');
  assert.ok(z.includes('return zorde_validate_protobuf_wire(') &&
    z.includes('decoded.unwrap(), rules, options, reject_unknown_fields'),
    'decoded fields must be passed through Zorde descriptor checks');
  assert.ok(t.includes('zorde_validate_protobuf_input from') &&
    t.includes('overlong_key') && t.includes('overlong_length') &&
    t.includes('key_over_original_limit') &&
    t.includes('length_over_original_limit') &&
    t.includes('malformed_input') && t.includes('rejected_unknown_input') &&
    t.includes('raw_before_mutation') && t.includes('raw_after_mutation'),
    'native guest fixtures must cover nonminimal prefixes, mutation, and raw errors');
}
originalWireAdmissionGuard(zorde,zordeTests);
checks++;
reject(()=>originalWireAdmissionGuard(zorde.replaceAll(
  'decode_message(input, options)','decode_message(new List<int>(), options)'),
  zordeTests));
reject(()=>originalWireAdmissionGuard(zorde.replace(
  'if decoded.is_err() then','if false then'),zordeTests));
reject(()=>originalWireAdmissionGuard(zorde.replace(
  'proto_zorde_issue(rejected, "wire-decode"',
  'proto_zorde_issue(rejected, "validated"'),zordeTests));
reject(()=>originalWireAdmissionGuard(zorde,
  zordeTests.replaceAll('length_over_original_limit',
    'length_over_canonical_limit')));
reject(()=>originalWireAdmissionGuard(zorde,
  zordeTests.replaceAll('raw_after_mutation','unchecked_stale_report')));
// Untrusted Protobuf encoding must enforce caller-supplied INPUT field-count
// and input-wire budgets before producing output, even when the legacy
// byte-only encoder could eventually fit. This is only a source-level
// mutation guard; the .ores tests still need a real compiler/runner.
function boundedEncodeAdmissionGuard(w,t) {
  const start=w.indexOf('pub fnc encode_message_bounded(');
  assert.ok(start>=0, 'bounded encoder entrypoint missing');
  const checked=w.slice(start, w.indexOf('// Compatibility-only lookup',start));
  const options=checked.indexOf('proto_encode_options_valid(output_limits)');
  const snapshot=checked.indexOf('proto_message_copy_bounded(message, input_limits)');
  const refuse=checked.indexOf('if detached.is_err() then');
  const encode=checked.indexOf('return encode_message(detached.unwrap(), output_limits);');
  assert.ok(options>=0 && snapshot>options && refuse>snapshot && encode>refuse,
    'input limits, snapshot and malformed-message denial must precede encoding');
  assert.ok(checked.includes('return Err(new ProtoError("invalid_message"'),
    'bounded encoder must fail closed when detached copy is invalid');
  for(const test of [
    'bounded_encoded.is_err()',
    'bounded_input_short.is_err()',
    'bounded_too_many.is_err()',
    'bounded_output_short.is_err()',
    'bounded_bad_options.is_err()',
    'bounded_invalid_other.is_err()',
    'bounded_mutated.is_err()',
    'mutable_encoder_bytes[0] = 2;'
  ]) {
    assert.ok(t.includes(test), 'native bounded-encoder regression missing: '+test);
  }
}
boundedEncodeAdmissionGuard(wire,protobufWireTests);
checks++;
reject(()=>boundedEncodeAdmissionGuard(wire.replace(
  'proto_message_copy_bounded(message, input_limits)', 'Ok(message)'),
  protobufWireTests));
reject(()=>boundedEncodeAdmissionGuard(wire.replace(
  'return encode_message(detached.unwrap(), output_limits);',
  'return encode_message(message, output_limits);'),protobufWireTests));
reject(()=>boundedEncodeAdmissionGuard(wire.replace(
  'if detached.is_err() then','if false then'),protobufWireTests));
reject(()=>boundedEncodeAdmissionGuard(wire.replace(
  'proto_encode_options_valid(output_limits)','true'),protobufWireTests));
reject(()=>boundedEncodeAdmissionGuard(wire,protobufWireTests.replace(
  'bounded_mutated.is_err()', 'bounded_mutated.is_ok()')));
reject(()=>boundedEncodeAdmissionGuard(wire,protobufWireTests.replace(
  'bounded_too_many.is_err()', 'bounded_too_many.is_ok()')));

// An approved source-local native receipt must distinguish CI attempts
// from approved manual executions. Run identifiers are context metadata,
// not compiler trust or an authorizing signature by themselves.
function nativeRunAttemptBindingGuard(h,w,fixtures) {
  for(const token of [
    'ZORDE_CI_RUN_ID:-',
    'ZORDE_CI_RUN_ATTEMPT:-',
    'ZORDE_CI_JOB:-',
    'CI run ID must be a positive decimal identifier',
    'CI run attempt must be a positive decimal identifier',
    'CI job identity must match the protected native job',
    '"ci_origin":',
    '"ci_run_id": os.environ.get("ZORDE_CI_RUN_ID") or None',
    '"ci_run_attempt":',
    '"ci_job": os.environ.get("ZORDE_CI_JOB") or None',
    'approved-manual-source-local'
  ]) {
    assert.ok(h.includes(token), 'native receipt CI provenance missing '+token);
  }
  const cmd=w.slice(w.indexOf('          env -i '));
  for(const token of [
    'ZORDE_CI_RUN_ID="${{ github.run_id }}"',
    'ZORDE_CI_RUN_ATTEMPT="${{ github.run_attempt }}"',
    'ZORDE_CI_JOB="${{ github.job }}"'
  ]) {
    assert.ok(cmd.includes(token), 'trusted workflow context binding missing '+token);
  }
  for(const token of [
    'expect_refusal ci-missing-attempt',
    'expect_refusal ci-missing-job',
    'expect_refusal ci-zero-run',
    'expect_refusal ci-negative-attempt',
    'expect_refusal ci-wrong-job',
    'expect_refusal ci-attempt-only'
  ]) {
    assert.ok(fixtures.includes(token), 'source-free negative receipt fixture missing '+token);
  }
}
nativeRunAttemptBindingGuard(nativeHarness,nativeWorkflow,refusalFixtures);
checks++;
reject(()=>nativeRunAttemptBindingGuard(nativeHarness,
  nativeWorkflow.replace('ZORDE_CI_RUN_ATTEMPT="${{ github.run_attempt }}"',
    'ZORDE_CI_RUN_ATTEMPT="1"'),refusalFixtures));
reject(()=>nativeRunAttemptBindingGuard(nativeHarness,
  nativeWorkflow.replace('ZORDE_CI_JOB="${{ github.job }}"',
    'ZORDE_CI_JOB="runner-contracts"'),refusalFixtures));
reject(()=>nativeRunAttemptBindingGuard(
  nativeHarness.replace('"ci_run_id": os.environ.get("ZORDE_CI_RUN_ID") or None',
    '"ci_run_id": None'),nativeWorkflow,refusalFixtures));
reject(()=>nativeRunAttemptBindingGuard(
  nativeHarness.replace('CI run attempt must be a positive decimal identifier',
    'CI run attempt validation disabled'),nativeWorkflow,refusalFixtures));
reject(()=>nativeRunAttemptBindingGuard(nativeHarness,nativeWorkflow,
  refusalFixtures.replace('expect_refusal ci-zero-run',
    'expect_success ci-zero-run')));
reject(()=>nativeRunAttemptBindingGuard(
  nativeHarness.replace('"approved-manual-source-local"',
    '"unclassified-run"'),nativeWorkflow,refusalFixtures));

console.log(JSON.stringify({status:'STATIC_PROTOBUF_CONTRACT_SHAPE_MATCH',
  checks,typespecCompilerExecuted:false,jsonSchemaValidatorExecuted:false,
  witParserExecuted:false,nativeGuestExecuted:false,canonicalAbiImplemented:false,
  zordeProofIssued:false,releaseReady:false}));
