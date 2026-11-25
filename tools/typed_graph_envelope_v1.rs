//! Host-only logical typed graph envelope reference, NOT a wire parser.
//! Closed schema: type 1 Node, type 2 Other; both have i32 and two
//! optional strong edges to Node. Document-local IDs are not actor handles.
//! No weak edges, dynamic type loading, callbacks or guest runtime claims.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Version, Schema, Root, Id, Type, Dangling, Unreachable, Budget, Allocation, Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: u32,
    pub type_id: u32,
    pub value: i32,
    pub next: Option<u32>,
    pub alias: Option<u32>,
}

pub struct Envelope<'a> {
    pub version: u32,
    pub root: u32,
    pub objects: &'a [Record],
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub nodes: usize,
    pub edges: usize,
    pub work: usize,
    pub output_bytes: usize,
    pub scratch_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Graph {
    root: u32,
    nodes: Vec<Record>,
}

impl Graph {
    pub fn root(&self) -> u32 { self.root }
    pub fn get(&self, id: u32) -> Option<&Record> { self.nodes.get(id as usize) }
    pub fn len(&self) -> usize { self.nodes.len() }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }

    // The output is an owned logical envelope table, not framing bytes.
    pub fn encode_table(&self, output_bytes: usize) -> Result<Vec<Record>, Reject> {
        let bytes = self.nodes.len().checked_mul(std::mem::size_of::<Record>())
            .ok_or(Reject::Budget)?;
        if bytes > output_bytes { return Err(Reject::Budget); }
        let mut result = Vec::new();
        result.try_reserve_exact(self.nodes.len()).map_err(|_| Reject::Allocation)?;
        result.extend_from_slice(&self.nodes);
        Ok(result)
    }
}

fn charge(used: &mut usize, amount: usize, limit: usize) -> Result<(), Reject> {
    let next = used.checked_add(amount).ok_or(Reject::Budget)?;
    if next > limit { return Err(Reject::Budget); }
    *used = next;
    Ok(())
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), Reject> {
    if cancelled.load(Ordering::Acquire) { Err(Reject::Cancelled) } else { Ok(()) }
}

/// The caller must authenticate the closed registered schema and bound framing
/// bytes before constructing this borrowed table. expected_root_type selects
/// the already admitted root descriptor, not a type name supplied by DATA.
/// Scratch is bounded separately; output allocation follows all validation.
/// Cancellation observed before the final check discards unpublished output.
pub fn decode(
    envelope: Envelope<'_>, expected_root_type: u32,
    limits: Limits, cancelled: &AtomicBool,
) -> Result<Graph, Reject> {
    check_cancel(cancelled)?;
    if envelope.version != 1 { return Err(Reject::Version); }
    if !matches!(expected_root_type, 1 | 2) { return Err(Reject::Schema); }
    let n = envelope.objects.len();
    if n == 0 || n > limits.nodes || n > 65536 { return Err(Reject::Budget); }
    let root = envelope.objects.get(envelope.root as usize).ok_or(Reject::Root)?;
    if root.type_id != expected_root_type { return Err(Reject::Type); }
    let output = n.checked_mul(std::mem::size_of::<Record>()).ok_or(Reject::Budget)?;
    let scratch = n.checked_mul(1 + std::mem::size_of::<usize>()).ok_or(Reject::Budget)?;
    if output > limits.output_bytes || scratch > limits.scratch_bytes {
        return Err(Reject::Budget);
    }

    let mut work = 0;
    let mut edges = 0;
    // Validate every record and edge, even those that turn out unreachable.
    for (i, record) in envelope.objects.iter().enumerate() {
        check_cancel(cancelled)?;
        charge(&mut work, 1, limits.work)?;
        if record.id as usize != i { return Err(Reject::Id); }
        if !matches!(record.type_id, 1 | 2) { return Err(Reject::Type); }
        for target in [record.next, record.alias].into_iter().flatten() {
            charge(&mut edges, 1, limits.edges)?;
            charge(&mut work, 1, limits.work)?;
            let destination = envelope.objects.get(target as usize).ok_or(Reject::Dangling)?;
            if destination.type_id != 1 { return Err(Reject::Type); }
        }
    }

    let mut seen = Vec::<u8>::new();
    seen.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
    seen.resize(n, 0);
    let mut queue = Vec::<usize>::new();
    queue.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
    seen[envelope.root as usize] = 1;
    queue.push(envelope.root as usize);
    let mut cursor = 0;
    while cursor < queue.len() {
        check_cancel(cancelled)?;
        charge(&mut work, 1, limits.work)?;
        let record = &envelope.objects[queue[cursor]];
        cursor += 1;
        for target in [record.next, record.alias].into_iter().flatten() {
            charge(&mut work, 1, limits.work)?;
            let target = target as usize;
            if seen[target] == 0 {
                seen[target] = 1;
                queue.push(target);
            }
        }
    }
    if queue.len() != n { return Err(Reject::Unreachable); }
    drop(seen);
    drop(queue);

    // Dense indices are the identity mapping. Cycles do not require pointer
    // patching, recursion or refcounts. Reserve before any output is visible.
    charge(&mut work, n, limits.work)?;
    check_cancel(cancelled)?;
    let mut nodes = Vec::new();
    nodes.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
    for record in envelope.objects {
        check_cancel(cancelled)?;
        nodes.push(*record);
    }
    check_cancel(cancelled)?;
    Ok(Graph { root: envelope.root, nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    const LIMITS: Limits = Limits {
        nodes: 65536, edges: 131072, work: 524288,
        output_bytes: 8 * 1024 * 1024, scratch_bytes: 8 * 1024 * 1024,
    };
    fn node(id: u32, next: Option<u32>, alias: Option<u32>) -> Record {
        Record { id, type_id: 1, value: id as i32, next, alias }
    }
    fn run(objects: &[Record], root: u32, limits: Limits) -> Result<Graph, Reject> {
        decode(Envelope { version: 1, root, objects }, 1, limits, &AtomicBool::new(false))
    }
    #[test]
    fn self_cycle_and_roundtrip() {
        let input = [node(0, Some(0), None)];
        let graph = run(&input, 0, LIMITS).unwrap();
        assert_eq!(graph.get(graph.root()).unwrap().next, Some(graph.root()));
        assert_eq!(graph.encode_table(LIMITS.output_bytes).unwrap(), input);
    }
    #[test]
    fn diamond_with_backedge_preserves_ids() {
        let input = [node(0, Some(1), Some(2)), node(1, Some(3), None),
            node(2, Some(3), None), node(3, Some(0), Some(3))];
        let graph = run(&input, 0, LIMITS).unwrap();
        assert_eq!(graph.len(), 4);
        assert_eq!(graph.get(1).unwrap().next, graph.get(2).unwrap().next);
        assert_eq!(graph.get(3).unwrap().next, Some(graph.root()));
        assert_eq!(graph.get(3).unwrap().alias, Some(3));
    }
    #[test]
    fn rejects_duplicate_out_of_order_dangling_and_unreachable() {
        assert_eq!(run(&[node(0, Some(1), None), node(0, None, None)], 0, LIMITS), Err(Reject::Id));
        assert_eq!(run(&[node(1, None, None)], 0, LIMITS), Err(Reject::Id));
        assert_eq!(run(&[node(0, Some(1), None)], 0, LIMITS), Err(Reject::Dangling));
        assert_eq!(run(&[node(0, None, None), node(1, None, None)], 0, LIMITS), Err(Reject::Unreachable));
    }
    #[test]
    fn registered_type_checks_even_on_hidden_edges() {
        let other = Record { type_id: 2, ..node(1, None, None) };
        assert_eq!(run(&[node(0, None, Some(1)), other], 0, LIMITS), Err(Reject::Type));
        assert_eq!(run(&[Record { type_id: 99, ..node(0, None, None) }], 0, LIMITS), Err(Reject::Type));
        let input = [Record { type_id: 2, ..node(0, None, None) }];
        assert_eq!(decode(Envelope { version: 1, root: 0, objects: &input }, 2,
            LIMITS, &AtomicBool::new(false)).unwrap().len(), 1);
    }
    #[test]
    fn exact_and_one_over_all_budgets() {
        let input = [node(0, Some(0), Some(0))];
        let exact = Limits { nodes: 1, edges: 2, work: 7,
            output_bytes: std::mem::size_of::<Record>(),
            scratch_bytes: 1 + std::mem::size_of::<usize>() };
        assert!(run(&input, 0, exact).is_ok());
        for limit in [
            Limits { nodes: 0, ..exact }, Limits { edges: 1, ..exact },
            Limits { work: 6, ..exact },
            Limits { output_bytes: exact.output_bytes - 1, ..exact },
            Limits { scratch_bytes: exact.scratch_bytes - 1, ..exact },
        ] { assert_eq!(run(&input, 0, limit), Err(Reject::Budget)); }
        assert_eq!(run(&input, 0, exact).unwrap().encode_table(exact.output_bytes - 1), Err(Reject::Budget));
    }
    #[test]
    fn version_root_and_cancellation_fail_closed() {
        let input = [node(0, None, None)];
        assert_eq!(run(&input, 1, LIMITS), Err(Reject::Root));
        assert_eq!(decode(Envelope { version: 2, root: 0, objects: &input }, 1,
            LIMITS, &AtomicBool::new(false)), Err(Reject::Version));
        assert_eq!(decode(Envelope { version: 1, root: 0, objects: &input }, 1,
            LIMITS, &AtomicBool::new(true)), Err(Reject::Cancelled));
    }
    #[test]
    fn ten_thousand_node_cycle_is_iterative() {
        let input: Vec<_> = (0..10000).map(|i| node(i, Some((i+1)%10000), None)).collect();
        let graph = run(&input, 0, LIMITS).unwrap();
        let mut cursor = graph.root();
        for i in 0..10000 {
            assert_eq!(cursor, i);
            cursor = graph.get(cursor).unwrap().next.unwrap();
        }
        assert_eq!(cursor, graph.root());
    }
}
