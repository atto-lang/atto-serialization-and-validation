import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,rmSync,mkdirSync,symlinkSync,writeFileSync,readFileSync,realpathSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join,resolve,dirname,basename,normalize,relative,isAbsolute} from 'node:path';

// Host-side fail-closed preflight only. This suite does NOT execute or
// authenticate the Oreslang compiler, any private code export, or native
// guest conformance. It detects accidental bypasses before checkout tests.
const script=resolve('scripts/test-zorde-generated-guest-source-local.sh');
const nodeGate=resolve('scripts/test-zorde-node-source-local.sh');
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


test('source-local Node gate refuses missing independent source snapshot and unsafe evidence',()=>{
  const bin=realpathSync(process.execPath);
  const tmp=mkdtempSync(join(tmpdir(),'zorde-node-gate-negative-'));
  try{
    const safe=join(tmp,'evidence');
    mkdirSync(safe,{mode:0o700});
    const link=join(tmp,'linked-evidence');
    symlinkSync(safe,link,'dir');
    const linkedEvidenceParent=join(tmp,'linked-evidence-parent');
    symlinkSync(tmp,linkedEvidenceParent,'dir');
    const linkedNodeParent=join(tmp,'linked-node-parent');
    symlinkSync(dirname(bin),linkedNodeParent,'dir');
    const good={
      ...process.env,
      NODE_BIN:bin,
      ZORDE_EVIDENCE_DIR:safe,
      EXPECTED_SOURCE_SHA:'a'.repeat(40),
      EXPECTED_SOURCE_TREE:'b'.repeat(40),
      EXPECTED_NODE_SHA256:createHash('sha256').update(readFileSync(bin)).digest('hex'),
    };
    const cases=[
      ['missing source approval',{...good,EXPECTED_SOURCE_SHA:''},/required reviewed input missing/],
      ['short source root tree',{...good,EXPECTED_SOURCE_TREE:'bad'},/invalid immutable source root tree/],
      ['invalid Node binary digest',{...good,EXPECTED_NODE_SHA256:'invalid'},/invalid independently approved Node binary digest/],
      ['symlink evidence',{...good,ZORDE_EVIDENCE_DIR:link},/evidence directory must be an absolute preexisting real directory/],
      ['symlinked evidence ancestor',{...good,ZORDE_EVIDENCE_DIR:join(linkedEvidenceParent,'evidence')},/path traverses a symbolic link/],
      ['symlinked Node executable ancestor',{...good,NODE_BIN:join(linkedNodeParent,basename(bin))},/path traverses a symbolic link/],
      ['unreviewed source SHA',good,/source commit does not match approval/],
    ];
    for(const [name,env,reason] of cases){
      const proc=spawnSync('bash',[nodeGate],{
        cwd:resolve('.'),env,encoding:'utf8',timeout:5000,maxBuffer:8192});
      assert.equal(proc.status,2,name+': '+proc.stderr);
      assert.match(proc.stderr,/ZORDE_NODE_QUALIFICATION_REFUSED:/,name);
      assert.match(proc.stderr,reason,name);
      assert.equal(proc.stdout,'',name+': must not issue a receipt');
    }
    const gate=readFileSync(nodeGate,'utf8');
    assert.match(gate,/O_NOFOLLOW/,'private test log must use no-follow opens');
    assert.match(gate,/set -C/,'private test log must use noclobber');
  } finally {rmSync(tmp,{recursive:true,force:true});}
});


test('source-local Node TAP gate rejects TODO and preserves exact passing receipt',()=>{
  // Independent synthetic checkout: never mirror or export private Oreslang
  // source. Exercises the actual Bash gate, not a JS copy of its parser.
  const root=mkdtempSync(join(tmpdir(),'zorde-node-tap-negative-'));
  const repo=join(root,'checkout');
  mkdirSync(join(repo,'scripts'),{recursive:true,mode:0o700});
  mkdirSync(join(repo,'tests','source_free'),{recursive:true,mode:0o700});
  const scriptBytes=readFileSync(nodeGate);
  writeFileSync(join(repo,'scripts','test-zorde-node-source-local.sh'),scriptBytes);
  const checkoutFile=join(repo,'tests','source_free','test_zorde_contract_ir.mjs');
  const guardFile=join(repo,'tests','source_free','test_zorde_native_guard.mjs');
  const bin=realpathSync(process.execPath);
  const binaryDigest=createHash('sha256').update(readFileSync(bin)).digest('hex');
  function git(args){
    const proc=spawnSync('git',args,{cwd:repo,encoding:'utf8',timeout:12000});
    assert.equal(proc.status,0,proc.stderr);
    return proc.stdout.trim();
  }
  function runCase(todo){
    // Node considers TODO tests non-failing, so a gate MUST compare the
    // actual pass count and refuse even when the process exit status is 0.
    writeFileSync(checkoutFile,[
      "import test from 'node:test';",
      "import assert from 'node:assert/strict';",
      "for(let i=0;i<80;i++) test('synthetic-'+i,",
      todo
        ? "  i===0 ? {todo:'intentionally incomplete'} : {},"
        : "  {},",
      "  ()=>assert.equal(i,i));",
      "",
    ].join('\n'));
    writeFileSync(guardFile,[
      "import test from 'node:test';",
      "import assert from 'node:assert/strict';",
      "for(let i=0;i<10;i++) test('synthetic-guard-'+i,()=>assert.ok(true));",
      "",
    ].join('\n'));
    git(['add','.']);
    git(['-c','user.name=Synthetic Check',
      '-c','user.email=synthetic@example.invalid',
      'commit','-qm',todo?'todo-fixture':'passed-fixture']);
    const evidence=join(root,todo?'todo-evidence':'passed-evidence');
    mkdirSync(evidence,{mode:0o700});
    const env={
      ...process.env,
      EXPECTED_SOURCE_SHA:git(['rev-parse','HEAD']),
      EXPECTED_SOURCE_TREE:git(['rev-parse','HEAD^{tree}']),
      EXPECTED_NODE_SHA256:binaryDigest,
      NODE_BIN:bin,
      ZORDE_EVIDENCE_DIR:evidence,
    };
    const run=spawnSync('bash',['scripts/test-zorde-node-source-local.sh'],{
      cwd:repo,env,encoding:'utf8',timeout:90000,maxBuffer:32768,
    });
    return {run,evidence};
  }
  try{
    git(['init','-q']);
    const accepted=runCase(false);
    assert.equal(accepted.run.status,0,accepted.run.stderr);
    const passed=JSON.parse(readFileSync(
      join(accepted.evidence,'zorde-node-real-suite-receipt.json'),'utf8'));
    assert.equal(passed.source_free_test_cases_passed,90);
    assert.equal(passed.release_ready,false);
    const denied=runCase(true);
    assert.equal(denied.run.status,2,denied.run.stderr);
    assert.match(denied.run.stderr,/ZORDE_NODE_QUALIFICATION_REFUSED:/);
    assert.equal(denied.run.stdout,'');
    assert.throws(()=>readFileSync(join(
      denied.evidence,'zorde-node-real-suite-receipt.json')));
    const log=readFileSync(join(denied.evidence,'zorde-node-real-suite.log'),'utf8');
    assert.match(log,/^# tests 90$/m);
    assert.match(log,/^# pass 89$/m);
    assert.match(log,/^# todo 1$/m);
  } finally {
    rmSync(root,{recursive:true,force:true});
  }
});
