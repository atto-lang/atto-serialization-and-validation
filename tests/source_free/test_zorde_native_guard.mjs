import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,rmSync,mkdirSync,symlinkSync,writeFileSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join,resolve,dirname,normalize,relative,isAbsolute} from 'node:path';

// Host-side fail-closed preflight only. This suite does NOT execute or
// authenticate the Oreslang compiler, any private code export, or native
// guest conformance. It detects accidental bypasses before checkout tests.
const script=resolve('scripts/test-zorde-generated-guest-source-local.sh');
const envBase={
  ...process.env,
  EXPECTED_SOURCE_SHA:'a'.repeat(40),
  EXPECTED_SOURCE_TREE:'b'.repeat(40),
  EXPECTED_COMPILER_SHA256:'c'.repeat(64),
  EXPECTED_CONTRACT_IR_SHA256:'d'.repeat(64),
  EXPECTED_PARITY_REPORT_SHA256:'e'.repeat(64),
  EXPECTED_VERIFICATION_SHA256:'f'.repeat(64),
  ORESLANG_BIN:'/a/nonexistent/approved/oreslang-binary',
};
function run(env) {
  return spawnSync('bash',[script],{
    cwd:resolve('.'),env,encoding:'utf8',timeout:4000,
    maxBuffer:16384
  });
}
const tmp=mkdtempSync(join(tmpdir(),'zorde-native-guard-'));
try {
  mkdirSync(join(tmp,'private-evidence'),{mode:0o700});
  const destination=join(tmp,'symlinked-evidence');
  symlinkSync(join(tmp,'private-evidence'),destination);
  const fakeExecutable=join(tmp,'inert-test-executable');
  const fakeBytes=Buffer.from('#!/bin/sh\nexit 99\n');
  writeFileSync(fakeExecutable,fakeBytes,{mode:0o700});
  const trustedTestDigest=createHash('sha256').update(fakeBytes).digest('hex');
  const base={...envBase,ZORDE_EVIDENCE_DIR:join(tmp,'private-evidence')};
  const withExecutable={...base,ORESLANG_BIN:fakeExecutable,
    EXPECTED_COMPILER_SHA256:trustedTestDigest};
  const cases=[
    ['no source SHA',{...base,EXPECTED_SOURCE_SHA:''},/required reviewed environment input missing/],
    ['malformed source SHA',{...base,EXPECTED_SOURCE_SHA:'not-hex'},/invalid expected source SHA/],
    ['malformed source tree',{...base,EXPECTED_SOURCE_TREE:'bad'},/invalid expected root tree/],
    ['malformed compiler digest',{...base,EXPECTED_COMPILER_SHA256:'0'},/invalid expected compiler digest/],
    ['absent independent IR digest',{...base,EXPECTED_CONTRACT_IR_SHA256:''},/required reviewed environment input missing/],
    ['malformed parity digest',{...base,EXPECTED_PARITY_REPORT_SHA256:'invalid'},/invalid externally reviewed evidence digest/],
    ['malformed consumer receipt digest',{...base,EXPECTED_VERIFICATION_SHA256:'invalid'},/invalid externally reviewed evidence digest/],
    ['evidence symlink',{...withExecutable,ZORDE_EVIDENCE_DIR:destination},/evidence directory must preexist/],
    ['wrong reviewed commit',withExecutable,/source SHA changed/],
    ['compiler missing',{...base},/compiler must be an absolute real executable/],
  ];
  for(const [label,env,reason] of cases) {
    test('native generated guest preflight refuses '+label,()=>{
      const proc=run(env);
      assert.equal(proc.status,2,label+': '+proc.stderr);
      assert.match(proc.stderr,/ZORDE_GENERATED_GUEST_REFUSED:/,label);
      assert.match(proc.stderr,reason,label);
      assert.equal(proc.stdout,'',label+' must not report successful native execution');
    });
  }
} finally {
  // Tests execute synchronously, so this immediately follows all cases on
  // registration; keep the fixtures through execution by cleanup on exit.
  process.once('exit',()=>rmSync(tmp,{recursive:true,force:true}));
}


// Compare the current trusted guest dependency graph with the reviewed pilot
// closure. This is a STATIC source test only; no compiler execution is implied.
test('native manifest covers exact recursive Zorde guest import closure',()=>{
  const expected=[
    'src/core/utf8.ores','src/core/utf8_scalar.ores','src/core/value.ores',
    'src/validation/validation.ores',
    'src/zorde/compiled_registry.ores','src/zorde/refinement_descriptor.ores',
    'src/zorde/scalar.ores','src/zorde/scalar_core.ores','src/zorde/zorde.ores',
    'tests/assert.ores',
  ];
  const roots=[
    'tests/assert.ores','src/zorde/compiled_registry.ores',
    'src/zorde/refinement_descriptor.ores','src/core/value.ores',
    'src/validation/validation.ores',
  ];
  const done=new Set();
  function visit(filename){
    assert.ok(!isAbsolute(filename) && !filename.startsWith('..'));
    if(done.has(filename))return;
    assert.ok(done.size<64,'dependency size limit exceeded');
    done.add(filename);
    const source=readFileSync(resolve(filename),'utf8');
    for(const line of source.split('\n')){
      if(!line.trimStart().startsWith('import '))continue;
      const match=line.match(/^\s*import\s+.+\s+from\s+(['"])(\.[^'"]+)\1;\s*$/);
      assert.ok(match,'unreviewed import grammar in '+filename);
      const target=normalize(join(dirname(filename),match[2]+'.ores'));
      assert.ok(!isAbsolute(target) && target!=='..' && !target.startsWith('..'+requirePathSeparator),
        'dependency escapes the approved source tree');
      visit(target.split('\\').join('/'));
    }
  }
  const requirePathSeparator=process.platform==='win32'?'\\':'/';
  for(const root of roots)visit(root);
  assert.deepEqual([...done].sort(),expected.sort());
  const guard=readFileSync(script,'utf8');
  assert.match(guard,/staticGuestImportClosure/);
  assert.match(guard,/len\(manifest\["files"\]\)==18\+len\(manifest\["staticGuestImportClosure"\]\)/);
  assert.match(guard,/static_guest_import_closure_sealed/);
});
