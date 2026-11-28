#!/usr/bin/env python3
"""Fail-closed raw Git worktree verifier. Source integrity only, NOT guest PASS."""
import argparse
import hashlib
import os
import stat
import subprocess
import sys
from pathlib import Path


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(
        ["git", "-C", str(root), *args], stderr=subprocess.DEVNULL
    )


def git_entries(raw: bytes, index: bool) -> dict[bytes, tuple[str, str]]:
    entries = {}
    for record in raw.split(b"\0"):
        if not record:
            continue
        try:
            meta, path = record.split(b"\t", 1)
            tokens = meta.decode("ascii").split()
            if index:
                mode, sha, stage = tokens
                if stage != "0":
                    raise ValueError("unmerged index stage")
            else:
                mode, kind, sha = tokens
                if kind != "blob":
                    raise ValueError("submodule/nonblob tree entry")
            if mode not in ("100644", "100755"):
                raise ValueError("symlink or unusual tracked file mode")
            if not path or path in entries:
                raise ValueError("duplicate/empty Git path")
            entries[path] = (mode, sha)
        except (UnicodeError, ValueError) as exc:
            raise ValueError("malformed Git tree/index record") from exc
    return entries


def verify(root: Path, expected_head: str, expected_tree: str) -> int:
    root = root.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("source checkout root is not a directory")
    for label, sha in (("source HEAD", expected_head), ("source tree", expected_tree)):
        if len(sha) != 40 or any(c not in "0123456789abcdef" for c in sha):
            raise ValueError(f"invalid pinned {label}")
    if git(root, "rev-parse", "--show-object-format").strip() != b"sha1":
        raise ValueError("unexpected Git object format")
    if Path(os.fsdecode(git(root, "rev-parse", "--show-toplevel").strip())).resolve() != root:
        raise ValueError("not the exact checkout root")
    if git(root, "rev-parse", "HEAD").decode().strip() != expected_head:
        raise ValueError("unexpected source HEAD")
    if git(root, "rev-parse", "HEAD^{tree}").decode().strip() != expected_tree:
        raise ValueError("unexpected source tree")

    tree = git_entries(git(root, "ls-tree", "-r", "-z", "--full-tree", "HEAD"), False)
    index = git_entries(git(root, "ls-files", "--stage", "-z"), True)
    if not tree or tree != index:
        raise ValueError("Git index differs from approved HEAD tree")
    for path, (mode, expected_sha) in tree.items():
        # Raw bytes: ignore .gitattributes clean/smudge filters and line endings.
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        with os.fdopen(os.open(root / os.fsdecode(path), flags), "rb") as stream:
            st = os.fstat(stream.fileno())
            if not stat.S_ISREG(st.st_mode):
                raise ValueError("tracked path is not an ordinary file")
            if bool(st.st_mode & 0o111) != (mode == "100755"):
                raise ValueError("tracked executable mode differs from HEAD")
            payload = stream.read()
        raw_sha = hashlib.sha1(
            b"blob " + str(len(payload)).encode("ascii") + b"\0" + payload
        ).hexdigest()
        if raw_sha != expected_sha:
            raise ValueError("raw tracked blob mismatch")

    for args in (
        ("ls-files", "-z", "--others", "--exclude-standard"),
        ("ls-files", "-z", "--others", "--ignored", "--exclude-standard"),
    ):
        if git(root, *args):
            raise ValueError("untracked or ignored worktree files present")
    return len(tree)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--tree", required=True)
    args = parser.parse_args()
    try:
        count = verify(args.root, args.head, args.tree)
        print(f"RAW_SERDE_SOURCE_CHECK_PASS tracked={count}")
        print("RAW_SOURCE_VERIFICATION_ONLY=PASS")
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print("RAW_SERDE_SOURCE_CHECK_FAIL: " + str(exc), file=sys.stderr)
        sys.exit(2)
