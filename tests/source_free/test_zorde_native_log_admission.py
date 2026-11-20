"""Synthetic native output admission regression against the committed heredoc.

No private compiler, TypeSpec authority, or guest is executed. This runs the
same inline output-opening/timeout code in an isolated scratch directory.
"""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile

if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    raise SystemExit('ZORDE_NATIVE_LOG_TEST_REFUSED: use python3 -I -S')

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / 'scripts/test-zorde-generated-guest-source-local.sh').read_text()
opening = '''  python3 -I -S - "$ORESLANG_BIN" "$root/artifacts/zorde-generated/registry-smoke.ores" <<'PY'\n'''
assert source.count(opening) == 1, 'native output gate moved or not isolated'
block = source.split(opening, 1)[1].split('\nPY\nstatus=$?', 1)[0]
assert 'os.O_EXCL' in block and 'os.O_NOFOLLOW' in block
assert 'os.O_DIRECTORY' in block and 'RLIMIT_FSIZE' in block

with tempfile.TemporaryDirectory(prefix='zorde-native-log-synthetic-') as tmp:
    base = Path(tmp)
    guest = base / 'synthetic-guest.ores'
    guest.write_text('synthetic fixture, not Oreslang code\n')

    compiler = base / 'good-compiler.sh'
    compiler.write_text('#!/bin/sh\nprintf "synthetic success\\n"\n')
    compiler.chmod(0o700)
    rejector = base / 'failure-compiler.sh'
    rejector.write_text('#!/bin/sh\nexit 7\n')
    rejector.chmod(0o700)
    verbose = base / 'large-output.sh'
    verbose.write_text('#!/bin/sh\nhead -c 20000000 /dev/zero\n')
    verbose.chmod(0o700)

    name = 'zorde-generated-smoke-native.log'
    count = 0

    def scenario(label, *, preexisting=None, compiler_path=None, directory_mode=0o700,
                 alias_directory=False, expect=True):
        global count
        scratch = base / label
        scratch.mkdir(mode=directory_mode)
        scratch.chmod(directory_mode)
        destination = scratch
        if alias_directory:
            destination = base / (label + '-alias')
            destination.symlink_to(scratch, target_is_directory=True)
        log = scratch / name
        protected = base / (label + '-protected')
        if preexisting == 'file':
            log.write_bytes(b'preserve')
        elif preexisting == 'fifo':
            os.mkfifo(log)
        elif preexisting == 'symlink':
            protected.write_bytes(b'preserve')
            log.symlink_to(protected)
        elif preexisting == 'dangling':
            log.symlink_to(protected)
        elif preexisting == 'hardlink':
            protected.write_bytes(b'preserve')
            os.link(protected, log)
        env = os.environ.copy()
        env.update(NATIVE_LOG_DIR=str(destination), NATIVE_LOG_NAME=name)
        # A 5s timeout makes an accidental FIFO redirection hang a hard failure.
        p = subprocess.run(
            [sys.executable, '-I', '-S', '-', str(compiler_path or compiler), str(guest)],
            input=block, cwd=base, env=env, capture_output=True,
            text=True, timeout=5, check=False)
        success = p.returncode == 0
        assert success == expect, (label, p.returncode, p.stderr[-600:])
        if expect:
            assert log.read_bytes() == b'synthetic success\n', label
            assert log.stat().st_mode & 0o077 == 0, label
        if preexisting == 'file':
            assert log.read_bytes() == b'preserve'
        if preexisting in ('symlink', 'hardlink'):
            assert protected.read_bytes() == b'preserve'
        if preexisting == 'dangling':
            assert not protected.exists()
        count += 1
        print('PASS', label, ('admitted' if expect else 'refused'))

    scenario('fresh-exclusive-private-output')
    scenario('preexisting-regular-file', preexisting='file', expect=False)
    scenario('existing-fifo-must-not-block', preexisting='fifo', expect=False)
    scenario('existing-symlink', preexisting='symlink', expect=False)
    scenario('dangling-symlink', preexisting='dangling', expect=False)
    scenario('existing-hardlink', preexisting='hardlink', expect=False)
    scenario('symlinked-evidence-directory', alias_directory=True, expect=False)
    scenario('group-accessible-evidence-directory', directory_mode=0o750, expect=False)
    scenario('compiler-nonzero', compiler_path=rejector, expect=False)
    scenario('compiler-output-over-16MiB', compiler_path=verbose, expect=False)
    assert count == 10
    print('ZORDE_NATIVE_LOG_ADMISSION_SYNTHETIC_10_OF_10_PASSED')
