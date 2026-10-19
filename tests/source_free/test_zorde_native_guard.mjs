import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,rmSync,mkdirSync,symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';

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
  const base={...envBase,ZORDE_EVIDENCE_DIR:join(tmp,'private-evidence')};
  const cases=[
    ['no source SHA',{...base,EXPECTED_SOURCE_SHA:''},/required reviewed environment input missing/],
    ['malformed source SHA',{...base,EXPECTED_SOURCE_SHA:'not-hex'},/invalid expected source SHA/],
    ['malformed source tree',{...base,EXPECTED_SOURCE_TREE:'bad'},/invalid expected root tree/],
    ['malformed compiler digest',{...base,EXPECTED_COMPILER_SHA256:'0'},/invalid expected compiler digest/],
    ['absent independent IR digest',{...base,EXPECTED_CONTRACT_IR_SHA256:''},/required reviewed environment input missing/],
    ['malformed parity digest',{...base,EXPECTED_PARITY_REPORT_SHA256:'invalid'},/invalid externally reviewed evidence digest/],
    ['malformed consumer receipt digest',{...base,EXPECTED_VERIFICATION_SHA256:'invalid'},/invalid externally reviewed evidence digest/],
    ['evidence symlink',{...base,ZORDE_EVIDENCE_DIR:destination},/compiler must be an absolute real executable|evidence directory must preexist/],
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
