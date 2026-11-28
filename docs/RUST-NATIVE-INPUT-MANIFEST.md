# Native input manifest Rust candidate

This host-only candidate captures the ten fixed authority/derived/source inputs,
nine reviewed corpus files, and 8–64 discovered guest dependencies. Each guest
is scanned and hashed from the same bounded read. File limits are 8 MiB and
128 MiB aggregate; BFS discovery depth 32, at most 256 import edges per module.
The same manifest schema is retained; post-execution checks re-derive the
entire manifest and compare it with the externally pinned original bytes.
Symlinks, hardlinks, FIFOs, wrong ownership, path traversal, unrecognized
import placement and unsafe evidence outputs are refused. The import scanner
is deliberately conservative and is not the compiler's parser or resolved
dependency closure. It may refuse valid source containing import tokens in
comments or strings. Exact real generated guest compatibility and isolated
runner integration remain required. The existing Python blocks and native
approval gates are unchanged. No actual compiler, guest or release proof.
