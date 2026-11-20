# Oreslang upstream compatibility — 2026-10-06

This repository is audited against the latest 10 PRs in `ores-truffle-oreslang/oreslang-source.java` (#405, #406, #408, #409, #410, #411, #412, #414, #415, #416).

## Upstream contract

- #405 `do select` / `do nb select` no-result syntax; legacy select remains compatible.
- #406 arm-local `return` inside `do select`, with `W-SELECT-RETURN` diagnostics.
- #408 surfaces those warnings through the CLI.
- #409 additive runtime `SelectPlan`; no source rewrite required and traditional select stays on `SelectSet`.
- #410 permits task-local data-only `Channel<T>` at async boundaries and makes explicit `rt take` consuming.
- #411 defines binding capabilities: `const` fixed/read-only, `val` = `const mut`, `let` rebindable/read-only, `let mut` rebindable/mutable; adds select-case bindings.
- #412 expands do-select control-flow/actor-domain/atomic-dispatch coverage.
- #414 hardens source continuations so selected arms/cleanup run once.
- #415 adds `define actor` parsing/isolation groundwork and host `ready`/`done`; source `spawn Worker()` lifecycle is still deferred.
- #416 shares immutable checked code images across same-process actor contexts; not cross-process or machine-code sharing.

## Repository impact

Serialization code should be the reference boundary for actor/RPC payloads: no function/pointer transport, explicit ownership operations, and task-local streaming channels only. #410 enables data-only Channel<T> async adapters but does not weaken actor-mailbox serialization.

## Integration policy

The ten PRs are not one linear compiler head. Several are stacked on separate bases and #415/#416 form a separate actor stack. Do not pin this repo to an arbitrary draft head and call it integrated. Apply source-compatible changes now, then advance compiler refs only after the required upstream stack is integrated and exact-head CI is green.

When source-org Actions cannot allocate a runner, mirror the exact source Git tree/blob SHAs into a funded test-org branch and run Actions there without private cross-org tokens or secrets.

## Guardrails

- `do select` / `do nb select` are no-result side-effecting forms; selected-arm returns are local/discarded.
- Traditional `select` and dynamic `select from cases` remain supported independently of #409.
- `cb select` / `nb cb select` are noncanonical.
- Preserve #411 binding distinctions; do not flatten everything to `val` or `let`.
- Keep source pointer-free; ownership is expressed by runtime ownership operations.
- Local channels and actor mailboxes have different payload rules; actor messages must remain serializable.
- #416 shares immutable checked code data only inside one OS process.
