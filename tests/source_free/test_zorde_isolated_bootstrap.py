"""Synthetic Bash-startup-injection regression; no private source/runner exported."""
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# This test and all admission/receipt heredocs must keep Python assertions
# enabled; -I ignores PYTHONOPTIMIZE and -S disables site customization.
if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    raise SystemExit('ZORDE_BOOTSTRAP_TEST_REFUSED: require python3 -I -S without -O')

for relative in (
    'scripts/test-zorde-contract-source-local.sh',
    'scripts/test-zorde-node-source-local.sh',
    'scripts/test-zorde-generated-guest-source-local.sh',
):
    checked=Path(__file__).resolve().parents[2]/relative
    heredocs=[line for line in checked.read_text().splitlines()
              if "python3" in line and "<<'PY'" in line]
    if not heredocs or any("python3 -I -S - " not in line for line in heredocs):
        raise SystemExit('ZORDE_BOOTSTRAP_TEST_REFUSED: unisolated Python heredoc in '+relative)
print('PASS all source-local Python admission/receipt heredocs enforce -I -S')

HERE = Path(__file__).resolve().parent
LAUNCHER = HERE.parents[1] / 'scripts/launch-zorde-contract-source-local.py'
GATE = HERE.parents[1] / 'scripts/test-zorde-contract-source-local.sh'

with tempfile.TemporaryDirectory(prefix='zorde-bootstrap-synthetic-') as temp:
    root = Path(temp)
    scripts = root / 'scripts'
    scripts.mkdir()
    shutil.copyfile(LAUNCHER, scripts / 'launch-zorde-contract-source-local.py')
    shutil.copyfile(GATE, scripts / 'test-zorde-contract-source-local.sh')
    marker = root / 'startup-ran'
    startup = root / 'startup.sh'
    startup.write_text("printf hijacked > " + str(marker) + "\n")
    env = os.environ.copy()
    env.update(BASH_ENV=str(startup), ENV=str(startup), EXPECTED_SOURCE_SHA='', EXPECTED_SOURCE_TREE='')

    def run(args, environment):
        return subprocess.run(args, cwd=root, env=environment, text=True,
                              capture_output=True, timeout=15)

    p = run([sys.executable, '-I', '-S', str(scripts/'launch-zorde-contract-source-local.py')], env)
    assert p.returncode == 2 and 'ZORDE_BOOTSTRAP_REFUSED' in p.stderr and not marker.exists(), p.stderr
    print('PASS isolated Python denies missing approval without executing BASH_ENV')

    p = run([sys.executable, str(scripts/'launch-zorde-contract-source-local.py')], os.environ.copy())
    assert p.returncode == 2 and 'Python isolated (-I) and no-site (-S)' in p.stderr
    print('PASS unisolated Python startup refused')

    env.update(EXPECTED_SOURCE_SHA='a'*40, EXPECTED_SOURCE_TREE='b'*40,
               EXPECTED_NODE_SHA256='c'*64, NODE_BIN='/not/a/real/node22',
               TJSV_ROOT=str(root/'public'), ZORDE_EVIDENCE_DIR=str(root/'evidence'))
    p = run([sys.executable, '-I', '-S', str(scripts/'launch-zorde-contract-source-local.py')], env)
    assert p.returncode == 2 and 'Node must be an absolute non-symlink executable' in p.stderr and not marker.exists(), p.stderr
    print('PASS startup hooks stripped; underlying Contract IR admission still refuses bad binary')

    p = run(['/usr/bin/bash', '--noprofile', '--norc', str(scripts/'test-zorde-contract-source-local.sh')], env)
    assert p.returncode == 2 and 'unsafe Bash startup environment' in p.stderr and marker.exists(), p.stderr
    print('PASS unsafe direct Bash detects startup hooks (after Bash has run them)')
    print('ZORDE_ISOLATED_BOOTSTRAP_SYNTHETIC_4_OF_4_PASSED')
