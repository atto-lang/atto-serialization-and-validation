"""Synthetic native-guest bootstrap admission tests, without compiler execution.

Uses copies of the exact committed gate/launcher inside a disposable local
directory. No private source leaves this private repository.
"""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile

if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    raise SystemExit('ZORDE_GUEST_BOOTSTRAP_TEST_REFUSED: require python3 -I -S')

SOURCE = Path(__file__).resolve().parents[2]
LAUNCHER = SOURCE / 'scripts/launch-zorde-generated-guest-source-local.py'
GATE = SOURCE / 'scripts/test-zorde-generated-guest-source-local.sh'


def run(args, root, env):
    return subprocess.run(args, cwd=root, env=env, text=True,
                          capture_output=True, timeout=15, check=False)


def refuse(proc, reason):
    assert proc.returncode == 2 and reason in proc.stderr, (
        proc.returncode, proc.stdout, proc.stderr)


with tempfile.TemporaryDirectory(prefix='zorde-generated-bootstrap-') as temp:
    root = Path(temp)
    scripts = root / 'scripts'
    scripts.mkdir(mode=0o700)
    guest = scripts / GATE.name
    launcher = scripts / LAUNCHER.name
    shutil.copyfile(GATE, guest)
    shutil.copyfile(LAUNCHER, launcher)
    startup = root / 'injected-startup.sh'
    marker = root / 'injected'
    startup.write_text('printf injected > ' + str(marker) + '\n')
    env = os.environ.copy()
    env.update({
        'BASH_ENV': str(startup),
        'ENV': str(startup),
        'NODE_OPTIONS': '--inspect',
        'PYTHONOPTIMIZE': '1',
        'GIT_DIR': str(root / 'malicious-gitdir'),
        'EXPECTED_SOURCE_SHA': 'a' * 40,
        'EXPECTED_SOURCE_TREE': 'b' * 40,
        'EXPECTED_COMPILER_SHA256': 'c' * 64,
        'EXPECTED_CONTRACT_IR_SHA256': 'd' * 64,
        'EXPECTED_PARITY_REPORT_SHA256': 'e' * 64,
        'EXPECTED_VERIFICATION_SHA256': 'f' * 64,
        'ORESLANG_BIN': '/does/not/exist/oreslang',
        'NODE_BIN': '/does/not/exist/node',
        'EXPECTED_NODE_SHA256': '1' * 64,
        'ZORDE_EVIDENCE_DIR': str(root / 'private-evidence'),
    })
    (root / 'private-evidence').mkdir(mode=0o700)

    # Positive control must prove the exact hostile fixture actually runs.
    direct = run(['/usr/bin/bash', '--noprofile', '--norc', '-c', ':'], root, env)
    assert direct.returncode == 0 and marker.read_bytes() == b'injected', direct.stderr
    marker.unlink()
    print('PASS hostile inherited BASH_ENV really executes in direct Bash')

    good = run([sys.executable, '-I', '-S', str(launcher)], root, env)
    refuse(good, 'compiler must be an absolute real executable')
    assert not marker.exists()
    print('PASS isolated guest entrypoint strips startup and rejects invalid compiler')

    direct = run(['/usr/bin/bash', '--noprofile', '--norc', str(guest)], root, env)
    refuse(direct, 'unsafe Bash startup environment')
    assert marker.read_bytes() == b'injected'
    marker.unlink()
    print('PASS unsafe direct Bash is recognized only after startup ran')

    for flags, reason in [
        ([], 'Python isolated (-I), no-site (-S), unoptimized'),
        (['-I', '-S', '-O'], 'unoptimized startup is required'),
        (['-I'], 'no-site (-S)'),
    ]:
        proc = run([sys.executable, *flags, str(launcher)], root, env)
        refuse(proc, reason)
        assert not marker.exists()
    print('PASS nonisolated, optimized and site-enabled Python startup refused')

    linked = scripts / 'linked-launcher.py'
    linked.symlink_to(launcher)
    proc = run([sys.executable, '-I', '-S', str(linked)], root, env)
    refuse(proc, 'without links')
    assert not marker.exists()
    print('PASS symlinked launcher leaf refused')

    alias = root / 'scripts-alias'
    alias.symlink_to(scripts, target_is_directory=True)
    proc = run([sys.executable, '-I', '-S', str(alias / LAUNCHER.name)], root, env)
    refuse(proc, 'ancestor cannot be a symlink')
    assert not marker.exists()
    print('PASS symlinked launcher/gate ancestor refused')

    saved = scripts / 'saved-guest.sh'
    guest.rename(saved)
    guest.symlink_to(saved)
    try:
        proc = run([sys.executable, '-I', '-S', str(launcher)], root, env)
        refuse(proc, 'without links')
        assert not marker.exists()
        print('PASS symlinked guest Bash script refused')
    finally:
        guest.unlink()
        saved.rename(guest)

    alias2 = scripts / 'hardlinked-guest.sh'
    os.link(guest, alias2)
    try:
        proc = run([sys.executable, '-I', '-S', str(launcher)], root, env)
        refuse(proc, 'without links')
        assert not marker.exists()
        print('PASS hardlinked guest Bash script refused')
    finally:
        alias2.unlink()

    env_bad = dict(env)
    env_bad['EXPECTED_COMPILER_SHA256'] = 'INVALID\nsecond-line'
    proc = run([sys.executable, '-I', '-S', str(launcher)], root, env_bad)
    refuse(proc, 'missing or invalid independently approved input')
    assert not marker.exists()
    print('PASS newline injection in independently approved input refused')

    # Node interpreter provenance must be bound before guest or corpus code.
    absent = dict(env)
    absent.pop('EXPECTED_NODE_SHA256')
    proc = run([sys.executable, '-I', '-S', str(launcher)], root, absent)
    refuse(proc, 'missing or invalid independently approved input: EXPECTED_NODE_SHA256')
    assert not marker.exists()
    print('PASS missing independently approved Node binary digest refused')

    malformed = dict(env)
    malformed['EXPECTED_NODE_SHA256'] = 'not-a-sha256'
    proc = run([sys.executable, '-I', '-S', str(launcher)], root, malformed)
    refuse(proc, 'invalid expected Node digest')
    assert not marker.exists()
    print('PASS malformed Node binary digest refused')

    fake_compiler = root / 'synthetic-compiler'
    fake_compiler.write_text('#!/usr/bin/env sh\nexit 0\n')
    fake_compiler.chmod(0o700)
    valid_paths = dict(env, ORESLANG_BIN=str(fake_compiler))

    proc = run([sys.executable, '-I', '-S', str(launcher)], root, valid_paths)
    refuse(proc, 'Node must be an absolute real executable')
    assert not marker.exists()
    print('PASS absent Node binary refused')

    fake_node = root / 'synthetic-node'
    fake_node.write_text('#!/usr/bin/env sh\nprintf v22.18.0\n')
    fake_node.chmod(0o700)
    # Prove the fake executable is a valid script, not an escaped-newline
    # pseudo-file that never reaches the system's interpreter.
    node_control = run([str(fake_node), '--version'], root, env)
    assert node_control.returncode == 0 and node_control.stdout.strip() == 'v22.18.0', node_control.stderr
    print('PASS synthetic Node executable actually runs in positive control')
    linked_node = root / 'linked-node'
    linked_node.symlink_to(fake_node)
    linked_paths = dict(valid_paths, NODE_BIN=str(linked_node))
    proc = run([sys.executable, '-I', '-S', str(launcher)], root, linked_paths)
    refuse(proc, 'Node must be an absolute real executable')
    assert not marker.exists()
    print('PASS symlinked Node binary refused')

    binaries = root / 'trusted-binaries'
    binaries.mkdir()
    nested_node = binaries / 'node'
    nested_node.write_text('#!/usr/bin/env sh\nprintf v22.18.0\n')
    nested_node.chmod(0o700)
    alias = root / 'binaries-alias'
    alias.symlink_to(binaries, target_is_directory=True)
    indirect = dict(valid_paths, NODE_BIN=str(alias / 'node'))
    proc = run([sys.executable, '-I', '-S', str(launcher)], root, indirect)
    refuse(proc, 'compiler/Node/evidence path traverses a symlink')
    assert not marker.exists()
    print('PASS Node executable behind symlinked ancestor refused')

    # These checks only exercise admission, never the guest compiler.
    print('ZORDE_GUEST_BOOTSTRAP_SYNTHETIC_17_OF_17_PASSED')
