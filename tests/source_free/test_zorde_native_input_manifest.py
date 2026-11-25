"""Run exact native guest manifest-builder Python against synthetic checkout trees.

No compiler, validator, real Oreslang guest, public-source mirror or token used.
The code under test is extracted directly from the committed private shell.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if not (sys.flags.isolated and sys.flags.no_site and sys.flags.optimize == 0):
    raise SystemExit('ZORDE_MANIFEST_TEST_REFUSED: require python3 -I -S')

SOURCE = Path(__file__).resolve().parents[2]
SHELL = (SOURCE / 'scripts/test-zorde-generated-guest-source-local.sh').read_text()
PREFIX = ('ROOT="$root" INPUT_MANIFEST="$before" python3 -I -S - <<\'PY\' ||\n'
          '  deny "unable to create independently digest-bound guest input manifest"\n')
assert SHELL.count(PREFIX) == 1
BLOCK = SHELL.split(PREFIX, 1)[1].split('\nPY\n', 1)[0]
assert 'bounded_input' in BLOCK and 'O_NOFOLLOW' in BLOCK
assert 'scripts/zorde-static-contract-ir.mjs' in BLOCK

FIXED = (
    'contracts/zorde/scalars.tsp',
    'contracts/zorde/scalars.schema.json',
    'scripts/zorde-static-contract-ir.mjs',
    'artifacts/zorde-scalars-parity.json',
    'artifacts/zorde-scalars-contract-ir.json',
    'artifacts/zorde-scalars-consumer-verification.json',
    'artifacts/zorde-generated/build-complete.json',
    'artifacts/zorde-generated/registry-plan.json',
    'artifacts/zorde-generated/registry-specs.ores',
    'artifacts/zorde-generated/registry-smoke.ores',
)


def put(root, path, content):
    p = root / path
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(content if isinstance(content, bytes) else content.encode())
    return p


def synthetic(root):
    for path in FIXED:
        put(root, path, 'synthetic fixture ' + path + '\n')
    base = 'contracts/zorde/scalar-instances'
    for schema, valid, invalid in (('Port', 3, 2), ('ShortLabel', 2, 2)):
        for lane, count in (('valid', valid), ('invalid', invalid)):
            for n in range(count):
                put(root, f'{base}/{schema}/{lane}/{n}.json', json.dumps(n))
    generated = 'artifacts/zorde-generated'
    put(root, generated + '/registry-smoke.ores',
        "import {D0} from './dep0';\n")
    put(root, generated + '/registry-specs.ores', 'synthetic spec\n')
    for n in range(8):
        content = f"import {{D{n+1}}} from './dep{n+1}';\n" if n < 7 else 'synthetic leaf\n'
        put(root, f'{generated}/dep{n}.ores', content)


def case(label, mutate=None, success=False):
    with tempfile.TemporaryDirectory(prefix='zorde-native-manifest-') as tmp:
        root = Path(tmp) / 'source'
        root.mkdir()
        synthetic(root)
        receipt = Path(tmp) / 'manifest.json'
        if mutate:
            mutate(root, receipt)
        env = dict(os.environ, ROOT=str(root), INPUT_MANIFEST=str(receipt))
        p = subprocess.run([sys.executable, '-I', '-S', '-c', BLOCK],
                           env=env, cwd=root, text=True,
                           capture_output=True, timeout=15, check=False)
        exists = receipt.is_file() and not receipt.is_symlink()
        if success:
            assert p.returncode == 0 and exists, (label, p.stderr)
            obj = json.loads(receipt.read_text())
            assert obj['schema'] == 'oreslang.zorde.native-guest-inputs/v1'
            assert len(obj['staticGuestImportClosure']) == 8
            assert len(obj['files']) == 27
            assert sorted(obj['staticGuestImportClosure']) == [
                f'artifacts/zorde-generated/dep{x}.ores' for x in range(8)]
            for file, want in obj['files'].items():
                assert hashlib.sha256((root / file).read_bytes()).hexdigest() == want
            print('PASS synthetic manifest binds 19 fixed inputs plus 8 imported modules')
        else:
            assert p.returncode != 0 and not exists, (
                label, p.returncode, p.stderr[-750:])
            print('PASS refused', label)


case('accepted synthetic graph', success=True)
case('symlinked guest import', lambda root, _: (
    (root / 'artifacts/zorde-generated/dep4.ores').unlink(),
    (root / 'artifacts/zorde-generated/dep4.ores').symlink_to(
        root / 'artifacts/zorde-generated/dep5.ores')))
case('hardlinked static bridge', lambda root, _: os.link(
    root / 'scripts/zorde-static-contract-ir.mjs', root / 'bridge-hardlink.mjs'))
case('FIFO corpus fixture', lambda root, _: (
    (root / 'contracts/zorde/scalar-instances/Port/valid/0.json').unlink(),
    os.mkfifo(root / 'contracts/zorde/scalar-instances/Port/valid/0.json')))
case('oversized imported Oreslang module', lambda root, _: put(
    root, 'artifacts/zorde-generated/dep7.ores', b'x' * (8 * 1024 * 1024 + 1)))


def substitute_directory(root, _):
    generated = root / 'artifacts/zorde-generated'
    new_dir = root / 'artifacts/zorde-staged'
    generated.rename(new_dir)
    generated.symlink_to(new_dir, target_is_directory=True)
case('symlinked generated source ancestor', substitute_directory)


def traversal(root, _):
    put(root, 'artifacts/zorde-generated/dep2.ores',
        "import {D} from '../../../../escape';\n")
case('guest import escaping source checkout', traversal)


def preexisting_manifest(root, path):
    target = root / 'outside-recipient'
    path.symlink_to(target)
case('preexisting manifest output symlink', preexisting_manifest)

print('ZORDE_NATIVE_INPUT_MANIFEST_SYNTHETIC_8_OF_8_PASSED')
