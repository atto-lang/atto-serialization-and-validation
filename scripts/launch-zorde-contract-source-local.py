#!/usr/bin/env python3
"""Safe, minimal-env entrypoint for the source-local private Zorde contract gate.

Invoke: python3 -I -S scripts/launch-zorde-contract-source-local.py
A direct `bash ...` call cannot prevent BASH_ENV from running *before*
the shell script starts. This bootstrap does not execute private code itself.
"""
import os
import sys
from pathlib import Path


def refuse(message):
    sys.stderr.write('ZORDE_BOOTSTRAP_REFUSED: ' + message + '\n')
    raise SystemExit(2)


if not sys.flags.isolated or not sys.flags.no_site:
    refuse('Python isolated (-I) and no-site (-S) startup are required')
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

script = Path(__file__).absolute().with_name('test-zorde-contract-source-local.sh')
if script.is_symlink() or not script.is_file():
    refuse('source-local gate must be an existing nonsymlink file')

# execve starts a fresh Bash with *only* the approved inputs: BASH_ENV, ENV,
# BASHOPTS, SHELLOPTS, exported Bash functions, LD_PRELOAD, injected Git/Python
# and Node startup variables cannot reach the shell's pre-execution phase.
os.execve('/usr/bin/bash', ('/usr/bin/bash', '--noprofile', '--norc', str(script)), allowed)
