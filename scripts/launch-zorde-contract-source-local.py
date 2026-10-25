#!/usr/bin/env python3
"""Safe, minimal-env entrypoint for the source-local private Zorde contract gate.

Invoke: python3 -I -S scripts/launch-zorde-contract-source-local.py
A direct `bash ...` call cannot prevent BASH_ENV from running *before*
the shell script starts. This bootstrap does not execute private code itself.
"""
import os
import stat
import sys
from pathlib import Path


def refuse(message):
    sys.stderr.write('ZORDE_BOOTSTRAP_REFUSED: ' + message + '\n')
    raise SystemExit(2)


if not sys.flags.isolated or not sys.flags.no_site or sys.flags.optimize:
    refuse('Python isolated (-I), no-site (-S), unoptimized startup is required')
if os.name != 'posix':
    refuse('approved Linux or POSIX source-local runner required')

required = (
    'EXPECTED_SOURCE_SHA', 'EXPECTED_SOURCE_TREE', 'EXPECTED_NODE_SHA256',
    'NODE_BIN', 'TJSV_ROOT', 'ZORDE_EVIDENCE_DIR',
)
allowed = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'}
for key in required:
    value = os.environ.get(key)
    if not value or '\x00' in value or '\n' in value or '\r' in value:
        refuse('missing or invalid independently approved input: ' + key)
    allowed[key] = value

# This path is checked without resolving symlinks: following an alias to
# the launcher or Bash gate before admission would execute unreviewed source.
# Reject symlinked *ancestors* too; checking only the leaf is insufficient.
launcher = Path(__file__).absolute()
script = launcher.with_name('test-zorde-contract-source-local.sh')
for path in (launcher, script):
    try:
        leaf = path.lstat()
        if not stat.S_ISREG(leaf.st_mode) or leaf.st_nlink != 1:
            refuse('source-local launcher/gate must be unique regular files, without links')
        for parent in path.parents:
            if not stat.S_ISDIR(parent.lstat().st_mode):
                refuse('source-local launcher/gate ancestor cannot be a symlink')
    except OSError:
        refuse('source-local launcher/gate path is inaccessible or unsafe')

# execve starts a fresh Bash with *only* the approved inputs: BASH_ENV, ENV,
# BASHOPTS, SHELLOPTS, exported Bash functions, LD_PRELOAD, injected Git/Python
# and Node startup variables cannot reach the shell's pre-execution phase.
os.execve('/usr/bin/bash', ('/usr/bin/bash', '--noprofile', '--norc', str(script)), allowed)
