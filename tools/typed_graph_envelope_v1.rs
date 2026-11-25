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

/// Restricted deterministic CBOR framing for the logical graph envelope:
/// [version, root, [[id, type_id, i32, next-or-null, alias-or-null], ...]].
/// Only definite arrays, shortest-form integers and null are admitted.
/// This is a proposed private profile, not a standard CBOR graph tag.
#[derive(Clone, Copy)]
pub struct CborLimits {
    pub graph: Limits,
    pub wire_bytes: usize,
    pub staging_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireReject {
    Graph(Reject), Truncated, Shape, NonCanonical, Range, Trailing,
}

impl From<Reject> for WireReject {
    fn from(value: Reject) -> Self { Self::Graph(value) }
}

struct CborReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl CborReader<'_> {
    fn take(&mut self) -> Result<u8, WireReject> {
        let byte = *self.bytes.get(self.pos).ok_or(WireReject::Truncated)?;
        self.pos += 1;
        Ok(byte)
    }

    fn argument(&mut self, expected_major: u8) -> Result<u64, WireReject> {
        let head = self.take()?;
        if head >> 5 != expected_major { return Err(WireReject::Shape); }
        let (length, minimum) = match head & 31 {
            x @ 0..=23 => return Ok(u64::from(x)),
            24 => (1, 24), 25 => (2, 256), 26 => (4, 65536),
            27 => (8, 4294967296),
            _ => return Err(WireReject::Shape),
        };
        let mut value = 0u64;
        for _ in 0..length { value = (value << 8) | u64::from(self.take()?); }
        if value < minimum { return Err(WireReject::NonCanonical); }
        Ok(value)
    }

    fn uint(&mut self) -> Result<u32, WireReject> {
        u32::try_from(self.argument(0)?).map_err(|_| WireReject::Range)
    }

    fn scalar(&mut self) -> Result<i32, WireReject> {
        let major = self.bytes.get(self.pos).ok_or(WireReject::Truncated)? >> 5;
        if major > 1 { return Err(WireReject::Shape); }
        let magnitude = i32::try_from(self.argument(major)?).map_err(|_| WireReject::Range)?;
        Ok(if major == 0 { magnitude } else { -1 - magnitude })
    }

    fn reference(&mut self) -> Result<Option<u32>, WireReject> {
        if self.bytes.get(self.pos) == Some(&0xf6) {
            self.pos += 1;
            Ok(None)
        } else { self.uint().map(Some) }
    }
}

/// Framing work is bounded by input bytes and the fixed five-field record
/// shape. Staging is explicitly separate from logical scratch/output budgets;
/// callers must reserve their combined peak, not assume any one cap is total.
/// The borrowed input is stable; no partially decoded graph escapes.
pub fn decode_cbor(
    bytes: &[u8], expected_root_type: u32,
    limits: CborLimits, cancelled: &AtomicBool,
) -> Result<Graph, WireReject> {
    check_cancel(cancelled)?;
    if bytes.len() > limits.wire_bytes { return Err(Reject::Budget.into()); }
    let mut reader = CborReader { bytes, pos: 0 };
    if reader.argument(4)? != 3 { return Err(WireReject::Shape); }
    let version = reader.uint()?;
    if version != 1 { return Err(Reject::Version.into()); }
    if !matches!(expected_root_type, 1 | 2) { return Err(Reject::Schema.into()); }
    let root = reader.uint()?;
    let count = usize::try_from(reader.argument(4)?).map_err(|_| WireReject::Range)?;
    if count == 0 || count > limits.graph.nodes || count > 65536
        || count > (bytes.len() - reader.pos) / 6 {
        return Err(Reject::Budget.into());
    }
    if root as usize >= count { return Err(Reject::Root.into()); }
    let staging = count.checked_mul(std::mem::size_of::<Record>()).ok_or(Reject::Budget)?;
    if staging > limits.staging_bytes || staging > limits.graph.output_bytes {
        return Err(Reject::Budget.into());
    }
    let mut records = Vec::new();
    records.try_reserve_exact(count).map_err(|_| Reject::Allocation)?;
    for _ in 0..count {
        check_cancel(cancelled)?;
        if reader.argument(4)? != 5 { return Err(WireReject::Shape); }
        records.push(Record {
            id: reader.uint()?, type_id: reader.uint()?, value: reader.scalar()?,
            next: reader.reference()?, alias: reader.reference()?,
        });
    }
    if reader.pos != bytes.len() { return Err(WireReject::Trailing); }
    decode(Envelope { version, root, objects: &records }, expected_root_type,
        limits.graph, cancelled).map_err(Into::into)
}

fn argument_size(value: u64) -> usize {
    match value { 0..=23 => 1, 24..=255 => 2, 256..=65535 => 3,
        65536..=4294967295 => 5, _ => 9 }
}

fn signed_argument(value: i32) -> (u8, u64) {
    if value >= 0 { (0, value as u64) } else { (1, (-1i64 - i64::from(value)) as u64) }
}

fn emit(out: &mut Vec<u8>, major: u8, value: u64) {
    let prefix = major << 5;
    match value {
        0..=23 => out.push(prefix | value as u8),
        24..=255 => { out.push(prefix | 24); out.push(value as u8); }
        256..=65535 => { out.push(prefix | 25); out.extend_from_slice(&(value as u16).to_be_bytes()); }
        65536..=4294967295 => { out.push(prefix | 26); out.extend_from_slice(&(value as u32).to_be_bytes()); }
        _ => { out.push(prefix | 27); out.extend_from_slice(&value.to_be_bytes()); }
    }
}

/// Only a privately constructed, validated Graph is encodable. Calculate
/// complete exact output length first, then allocate once and emit canonical
/// bytes. Cancellation before the final check drops the unpublished buffer.
pub fn encode_cbor(graph: &Graph, wire_bytes: usize,
                   cancelled: &AtomicBool) -> Result<Vec<u8>, WireReject> {
    check_cancel(cancelled)?;
    let mut size = 2 + argument_size(u64::from(graph.root))
        + argument_size(graph.nodes.len() as u64);
    if size > wire_bytes { return Err(Reject::Budget.into()); }
    for record in &graph.nodes {
        check_cancel(cancelled)?;
        let mut cost = 1 + argument_size(u64::from(record.id))
            + argument_size(u64::from(record.type_id))
            + argument_size(signed_argument(record.value).1);
        for target in [record.next, record.alias] {
            cost += target.map_or(1, |id| argument_size(u64::from(id)));
        }
        charge(&mut size, cost, wire_bytes)?;
    }
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| Reject::Allocation)?;
    emit(&mut out, 4, 3);
    emit(&mut out, 0, 1);
    emit(&mut out, 0, u64::from(graph.root));
    emit(&mut out, 4, graph.nodes.len() as u64);
    for record in &graph.nodes {
        check_cancel(cancelled)?;
        emit(&mut out, 4, 5);
        emit(&mut out, 0, u64::from(record.id));
        emit(&mut out, 0, u64::from(record.type_id));
        let (major, magnitude) = signed_argument(record.value);
        emit(&mut out, major, magnitude);
        for target in [record.next, record.alias] {
            match target { Some(id) => emit(&mut out, 0, u64::from(id)), None => out.push(0xf6) }
        }
    }
    check_cancel(cancelled)?;
    debug_assert_eq!(out.len(), size);
    Ok(out)
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
    const CBOR_LIMITS: CborLimits = CborLimits {
        graph: LIMITS, wire_bytes: 1024 * 1024, staging_bytes: 8 * 1024 * 1024,
    };

    #[test]
    fn cbor_exact_cyclic_wire_and_roundtrip() {
        let graph = run(&[node(0, Some(0), None)], 0, LIMITS).unwrap();
        let cancelled = AtomicBool::new(false);
        let bytes = encode_cbor(&graph, 10, &cancelled).unwrap();
        assert_eq!(bytes, [0x83, 1, 0, 0x81, 0x85, 0, 1, 0, 0, 0xf6]);
        assert_eq!(decode_cbor(&bytes, 1, CBOR_LIMITS, &cancelled).unwrap(), graph);
        assert_eq!(encode_cbor(&graph, 9, &cancelled), Err(Reject::Budget.into()));
        let exact = CborLimits { wire_bytes: bytes.len(),
            staging_bytes: std::mem::size_of::<Record>(), ..CBOR_LIMITS };
        assert!(decode_cbor(&bytes, 1, exact, &cancelled).is_ok());
        assert_eq!(decode_cbor(&bytes, 1, CborLimits { wire_bytes: 9, ..exact }, &cancelled),
            Err(Reject::Budget.into()));
        assert_eq!(decode_cbor(&bytes, 1, CborLimits { staging_bytes: exact.staging_bytes - 1,
            ..exact }, &cancelled), Err(Reject::Budget.into()));
    }

    #[test]
    fn cbor_rejects_truncation_nonminimal_indefinite_and_trailing() {
        let bytes = [0x83, 1, 0, 0x81, 0x85, 0, 1, 0, 0, 0xf6];
        let cancelled = AtomicBool::new(false);
        for length in 0..bytes.len() {
            assert!(decode_cbor(&bytes[..length], 1, CBOR_LIMITS, &cancelled).is_err());
        }
        let mut extra = bytes.to_vec(); extra.push(0);
        assert_eq!(decode_cbor(&extra, 1, CBOR_LIMITS, &cancelled), Err(WireReject::Trailing));
        let nonminimal = [0x83, 0x18, 1, 0, 0x81, 0x85, 0, 1, 0, 0, 0xf6];
        assert_eq!(decode_cbor(&nonminimal, 1, CBOR_LIMITS, &cancelled), Err(WireReject::NonCanonical));
        let indefinite = [0x83, 1, 0, 0x9f, 0x85, 0, 1, 0, 0, 0xf6, 0xff];
        assert_eq!(decode_cbor(&indefinite, 1, CBOR_LIMITS, &cancelled), Err(WireReject::Shape));
        let mut bad_field = bytes; bad_field[9] = 0xf5;
        assert_eq!(decode_cbor(&bad_field, 1, CBOR_LIMITS, &cancelled), Err(WireReject::Shape));
    }

    #[test]
    fn cbor_signed_i32_boundaries_and_graph_type_denial() {
        let cancelled = AtomicBool::new(false);
        for value in [i32::MIN, -25, -24, -1, 0, 23, 24, 255, 256, i32::MAX] {
            let graph = run(&[Record { value, ..node(0, Some(0), None) }], 0, LIMITS).unwrap();
            let bytes = encode_cbor(&graph, CBOR_LIMITS.wire_bytes, &cancelled).unwrap();
            assert_eq!(decode_cbor(&bytes, 1, CBOR_LIMITS, &cancelled).unwrap(), graph);
        }
        let out_of_range = [0x83, 1, 0, 0x81, 0x85, 0, 1, 0x1a, 0x80, 0, 0, 0, 0, 0xf6];
        assert_eq!(decode_cbor(&out_of_range, 1, CBOR_LIMITS, &cancelled), Err(WireReject::Range));
        let dangling = [0x83, 1, 0, 0x81, 0x85, 0, 1, 0, 1, 0xf6];
        assert_eq!(decode_cbor(&dangling, 1, CBOR_LIMITS, &cancelled), Err(Reject::Dangling.into()));
        let wrong_type = [0x83, 1, 0, 0x81, 0x85, 0, 2, 0, 0xf6, 0xf6];
        assert_eq!(decode_cbor(&wrong_type, 1, CBOR_LIMITS, &cancelled), Err(Reject::Type.into()));
        let graph = run(&[node(0, None, None)], 0, LIMITS).unwrap();
        cancelled.store(true, Ordering::Release);
        assert_eq!(encode_cbor(&graph, 100, &cancelled), Err(Reject::Cancelled.into()));
        assert_eq!(decode_cbor(&dangling, 1, CBOR_LIMITS, &cancelled), Err(Reject::Cancelled.into()));
    }

    #[test]
    fn cbor_large_cycle_has_byte_identical_reencoding() {
        let input: Vec<_> = (0..10000).map(|i| node(i, Some((i+1)%10000), None)).collect();
        let graph = run(&input, 0, LIMITS).unwrap();
        let cancelled = AtomicBool::new(false);
        let bytes = encode_cbor(&graph, CBOR_LIMITS.wire_bytes, &cancelled).unwrap();
        let decoded = decode_cbor(&bytes, 1, CBOR_LIMITS, &cancelled).unwrap();
        assert_eq!(decoded, graph);
        assert_eq!(encode_cbor(&decoded, bytes.len(), &cancelled).unwrap(), bytes);
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
