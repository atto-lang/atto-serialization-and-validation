# Candidate native ownership probes (not executed)

These isolated guest candidates intentionally attempt storing borrowed Value
references in a mutable container. Do not add them to tests/main.ores until
compiler semantics are established. Use the independently approved compiler
and exact reviewed tree. Run each separately with a bounded timeout.

- self-cycle: if the ownership checker permits the edge, both admission paths
  must refuse and print the runtime marker.
- mutual-cycle: same, with two active nodes.
- shared-subgraph: if the compiler permits sharing, both exact/one-over
  expanded-output checks and the successful DAG validation must execute.

A targeted compiler ownership rejection is evidence only of static denial
for that candidate, not runtime cycle protection. A syntax/import error,
generic failure, runner nonadmission or timeout is neither. If the compiler
copies instead of shares, require separate identity/alias evidence before
classifying the runtime result as an actual cycle or shared DAG. Never
disable borrow checking to manufacture an apparent success.

Current status: NOT_EXECUTED. No native receipt or compiler capability claim.
