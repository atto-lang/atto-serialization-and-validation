#!/usr/bin/env python3
"""Verify physical source bytes against the reviewed Git tree (not its index).

Git status and diff are not sufficient when skip-worktree/assume-unchanged or
racy index metadata conceal modified source bytes. This verifier intentionally
rejects transformed working trees and submodules: native evidence requires an
exact, isolated, byte-for-byte checkout of an explicitly approved snapshot.
"""
import hashlib
import os
from pathlib import Path
import stat
import subprocess
import sys

MAX_FILES = 10000
MAX_TOTAL_BYTES = 512 * 1024 * 1024


def fail(message: str) -> None:
    raise ValueError(message)


def git_bytes(*args: str) -> bytes:
    return subprocess.run(
        ["git", *args], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    ).stdout


def hash_file(path: Path, expected_mode: bytes, algorithm: str) -> str:
    before = path.lstat()
    # A tracked symlink has a stable Git blob but may point outside this
    # checkout at compiler time. No source-local native receipt can attest
    # the referenced target bytes from this tree. Reject symlinks entirely.
    if expected_mode == b"120000":
        fail("tracked symlinks are forbidden in source-local native evidence")

    if expected_mode not in (b"100644", b"100755"):
        fail("unsupported tracked mode; submodules require separate attestation")
    if not stat.S_ISREG(before.st_mode):
        fail("tracked regular file changed type")
    if bool(before.st_mode & 0o111) != (expected_mode == b"100755"):
        fail("tracked executable mode changed")

    digest = hashlib.new(algorithm)
    digest.update(b"blob " + str(before.st_size).encode("ascii") + b"\0")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags)
    try:
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode) or (
            opened.st_dev, opened.st_ino, opened.st_size
        ) != (before.st_dev, before.st_ino, before.st_size):
            fail("tracked file changed during open")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            while True:
                chunk = stream.read(1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
        after = os.fstat(fd)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns,
                opened.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_size,
                                       after.st_mtime_ns, after.st_ctime_ns):
            fail("tracked file changed during hashing")
    finally:
        os.close(fd)
    return digest.hexdigest()


def verify(expected_commit: str, expected_tree: str) -> None:
    if not (len(expected_commit) == len(expected_tree) == 40 and
            all(c in "0123456789abcdef" for c in expected_commit + expected_tree)):
        fail("invalid expected SHA-1 commit or tree")
    if git_bytes("rev-parse", "HEAD").strip().decode() != expected_commit:
        fail("HEAD does not equal reviewed commit")
    if git_bytes("rev-parse", "HEAD^{tree}").strip().decode() != expected_tree:
        fail("root tree does not equal reviewed tree")
    root = Path(os.fsdecode(git_bytes("rev-parse", "--show-toplevel").strip())).resolve()
    # Git porcelain/status deliberately hides ignored files. In an evidence
    # checkout, ignored files can change compiler import discovery, scripts,
    # toolchain resolution or generated outputs without changing the Git tree.
    # Reject them entirely rather than silently equating 'clean' with exact.
    ignored = git_bytes("ls-files", "--others", "--ignored",
                        "--exclude-standard", "-z")
    if ignored:
        fail("ignored untracked files exist in source checkout")
    # This verifier is run before any checked-out script on the trusted
    # worker. Ordinary untracked files may influence import discovery or tools
    # even when all tracked blobs match. The later shell status check is
    # defense in depth, not a substitute for this pre-execution refusal.
    untracked = git_bytes("ls-files", "--others", "--exclude-standard", "-z")
    if untracked:
        fail("untracked files exist in source checkout")
    entries = git_bytes("ls-tree", "-rz", "--full-tree", "HEAD").split(b"\0")
    total = 0
    count = 0
    for entry in entries:
        if not entry:
            continue
        count += 1
        if count > MAX_FILES:
            fail("tracked file-count budget exceeded")
        metadata, sep, name = entry.partition(b"\t")
        if not sep or not name or b"\0" in name:
            fail("malformed Git tree entry")
        mode, object_type, oid = metadata.split(b" ")
        if object_type != b"blob" or len(oid) != 40:
            fail("unattested Git tree object or submodule")
        # Refuse the mode before even resolving the path. A checkout can
        # contain an untrusted symlink target despite correct Git blob IDs.
        if mode == b"120000":
            fail("tracked symlinks are forbidden in source-local native evidence")
        parts = name.split(b"/")
        if any(p in (b"", b".", b"..") for p in parts):
            fail("invalid Git tree path")
        path = root / os.fsdecode(name)
        # An untracked directory symlink could redirect otherwise correct
        # tracked leaf bytes outside the isolated checkout. Git cannot track
        # children beneath a symlink directory in the approved tree.
        parent = path.parent
        while parent != root:
            if not stat.S_ISDIR(parent.lstat().st_mode):
                fail("tracked parent directory is not a real directory")
            parent = parent.parent
        st = path.lstat()
        total += st.st_size
        if total > MAX_TOTAL_BYTES:
            fail("tracked byte budget exceeded")
        if hash_file(path, mode, "sha1").encode("ascii") != oid:
            fail("working-tree blob differs from reviewed Git tree")
    if count == 0:
        fail("cannot attest an empty source checkout")


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: verify-zorde-checkout.py COMMIT TREE", file=sys.stderr)
        return 2
    try:
        verify(sys.argv[1], sys.argv[2])
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print("Zorde source-byte integrity refused:", exc, file=sys.stderr)
        return 2
    print("Zorde source-byte integrity: PASS (exact Git blobs; no compiler executed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
