# Rust source integrity candidate

The standalone Linux Rust candidate verifies exact commit/tree identity, raw
Git blob hashes, regular file modes and physical directory entries before and
after hashing. It rejects ignored, staged or untracked extra files, FIFOs,
symlinks (including tracked symlinks), submodules, extra directories, malformed
paths, and excessive entry/byte counts. The current reviewed tree contains no
tracked symlinks or submodules; refusal is intentionally stricter than the
legacy Python verifier. Raw non-UTF-8 Git path bytes are preserved on Linux.
Trusted system Git hashes file streams without filters or object writes.
No-follow/nonblocking opens and metadata checks detect persistent substitutions;
this is not an atomic snapshot against a malicious same-UID process or
compromised Git/object database. The existing native harness and Python
verifier remain active until exact-head and synthetic checkout tests execute
and integration is reviewed. No policy exception or guest proof.
