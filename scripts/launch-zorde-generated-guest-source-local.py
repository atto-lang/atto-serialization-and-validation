#!/usr/bin/env python3
"""Isolated, minimal-environment entrypoint for native Zorde guest qualification.

Invoke: python3 -I -S scripts/launch-zorde-generated-guest-source-local.py
No direct Bash invocation can prevent inherited BASH_ENV from executing first.
This launcher does not run the compiler and cannot certify its dependency closure.
"""
import os
import stat
import sys
from pathlib import Path


def refuse(message):
    sys.stderr.write('ZORDE_GUEST_BOOTSTRAP_REFUSED: ' + message + '\n')
    raise SystemExit(2)


if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    refuse('Python isolated (-I), no-site (-S), unoptimized startup is required')
if os.name != 'posix' or not Path('/usr/bin/bash').is_file():
    refuse('approved Linux/POSIX runner with /usr/bin/bash required')

required = (
    'EXPECTED_SOURCE_SHA', 'EXPECTED_SOURCE_TREE',
    'EXPECTED_COMPILER_SHA256', 'EXPECTED_CONTRACT_IR_SHA256',
    'EXPECTED_PARITY_REPORT_SHA256', 'EXPECTED_VERIFICATION_SHA256',
    'ORESLANG_BIN', 'NODE_BIN', 'EXPECTED_NODE_SHA256',
    'ZORDE_EVIDENCE_DIR',
)
allowed = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'}
for key in required:
    value = os.environ.get(key)
    if not value or '\x00' in value or '\n' in value or '\r' in value:
        refuse('missing or invalid independently approved input: ' + key)
    allowed[key] = value

launcher = Path(__file__).absolute()
gate = launcher.with_name('test-zorde-generated-guest-source-local.sh')
# A symlink anywhere in the launcher/gate path can defeat a leaf-only check.
# Link counts also reject hardlinked aliases of either script.
for path in (launcher, gate):
    try:
        leaf = path.lstat()
        if not stat.S_ISREG(leaf.st_mode) or leaf.st_nlink != 1:
            refuse('source-local launcher/gate must be unique regular files, without links')
        for parent in path.parents:
            if not stat.S_ISDIR(parent.lstat().st_mode):
                refuse('source-local launcher/gate ancestor cannot be a symlink')
    except OSError:
        refuse('source-local launcher/gate path is inaccessible or unsafe')

# Bash is created only after the original inherited environment is discarded.
# Race-free replacement of script paths requires an exclusive source checkout;
# this launcher does not claim an adversarial co-writer is isolated.
os.execve('/usr/bin/bash', ('/usr/bin/bash', '--noprofile', '--norc', str(gate)), allowed)
