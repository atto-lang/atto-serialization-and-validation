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
