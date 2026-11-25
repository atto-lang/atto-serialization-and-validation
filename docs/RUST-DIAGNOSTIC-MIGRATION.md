# Rust visibility diagnostic migration candidate

This standalone Rust candidate is deliberately separate from the currently
protected source-local harness until actual diagnostic wording and the full
legacy regression set are verified. It combines same-line denial binding with
exact registry owner qualification, bounds physical log bytes before stripping
formatting, rejects malformed UTF-8 and quote-spliced symbols, and rejects
mixed or repeated error diagnostics. The grammar is intentionally narrower
than the legacy visibility-only classifier (isolated multiline forms are not
accepted by the combined gate). Existing Python tools, tests, and native
approval gates remain in place. No policy exception is introduced. Migration
requires equivalent or explicitly stricter tested outcomes for all remaining
legacy cases, CLI and source-local refusal tests, then atomic call-site
replacement and removal. Native compiler diagnostic evidence remains separate.

The follow-up matrix explicitly distinguishes the old independent checker
from the intended combined gate: unqualified `entries` and multiline-only
private errors are not accepted. Physical log limits are 64 KiB and 4096
bytes per line; unlike the old secondary checker, any overlong line fails
closed. SGR decorations are allowed only around a whole line, not inside
a subject or phrase. This is a curated regression migration, not a complete
mechanical differential comparison or native compiler diagnostic receipt.
