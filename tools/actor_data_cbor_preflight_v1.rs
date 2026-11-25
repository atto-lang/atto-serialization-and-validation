//! Host-only deterministic actor DATA CBOR subset preflight.
//! Preflight is allocation-free; typed decode allocates only after charging.
//! No descriptor inference, no maps/tags/indefinite items.
//! Includes opt-in typed decoding using the same reader and cumulative meter.
//! Not a general RFC 8949 implementation.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject { Limits, Truncated, Unsupported, NonCanonical, Range, Utf8, Budget, Trailing, Shape, Allocation }

#[derive(Clone, Copy)]
pub struct Limits {
    pub wire: usize, pub expanded: usize, pub nodes: usize,
    pub elements: usize, pub work: usize, pub depth: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub expanded: usize, pub nodes: usize, pub elements: usize, pub work: usize,
}

/// Trusted registration-time descriptor, never obtained from DATA.
#[derive(Clone, Copy)]
pub enum Shape<'a> { Bool, I64, F64, Text, List(&'a Shape<'a>) }

#[derive(Debug, PartialEq)]
pub enum Value { Bool(bool), I64(i64), F64(f64), Text(String), List(Vec<Value>) }

struct Reader<'a> {
    bytes: &'a [u8], pos: usize, limits: Limits, usage: Usage,
}

fn charge(counter: &mut usize, amount: usize, cap: usize) -> Result<(), Reject> {
    let next = counter.checked_add(amount).ok_or(Reject::Budget)?;
    if next > cap { return Err(Reject::Budget); }
    *counter = next;
    Ok(())
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Reject> {
        let end = self.pos.checked_add(n).ok_or(Reject::Budget)?;
        if end > self.bytes.len() { return Err(Reject::Truncated); }
        charge(&mut self.usage.work, n, self.limits.work)?;
        let start = self.pos;
        self.pos = end;
        Ok(&self.bytes[start..end])
    }
    fn arg(&mut self, ai: u8) -> Result<u64, Reject> {
        let (n, min) = match ai {
            0..=23 => return Ok(u64::from(ai)),
            24 => (1, 24), 25 => (2, 256), 26 => (4, 65536),
            27 => (8, 4294967296), _ => return Err(Reject::Unsupported),
        };
        let mut v = 0u64;
        for b in self.take(n)? { v = (v << 8) | u64::from(*b); }
        if v < min { return Err(Reject::NonCanonical); }
        Ok(v)
    }
    fn item(&mut self, depth: usize, shape: Option<&Shape<'_>>) -> Result<Option<Value>, Reject> {
        if depth > self.limits.depth { return Err(Reject::Budget); }
        charge(&mut self.usage.nodes, 1, self.limits.nodes)?;
        charge(&mut self.usage.expanded, 32, self.limits.expanded)?;
        charge(&mut self.usage.work, 1, self.limits.work)?;
        let head = self.take(1)?[0];
        let major = head >> 5;
        let ai = head & 31;
        if let Some(expected) = shape {
            let matches = match expected {
                Shape::Bool => major == 7 && (ai == 20 || ai == 21),
                Shape::I64 => major == 0 || major == 1,
                Shape::F64 => major == 7 && ai == 27,
                Shape::Text => major == 3,
                Shape::List(_) => major == 4,
            };
            if !matches { return Err(Reject::Shape); }
        }
        let value = match major {
            // Both positive and negative signed i64 boundaries.
            0 | 1 => {
                let v = self.arg(ai)?;
                if v > i64::MAX as u64 { return Err(Reject::Range); }
                shape.map(|_| Value::I64(if major == 0 { v as i64 } else { -1 - v as i64 }))
            }
            3 => {
                let n = usize::try_from(self.arg(ai)?).map_err(|_| Reject::Range)?;
                let expanded = n.checked_mul(4).ok_or(Reject::Budget)?;
                charge(&mut self.usage.expanded, expanded, self.limits.expanded)?;
                charge(&mut self.usage.work, n, self.limits.work)?;
                if shape.is_some() { charge(&mut self.usage.work, n, self.limits.work)?; }
                let text = std::str::from_utf8(self.take(n)?).map_err(|_| Reject::Utf8)?;
                if shape.is_some() {
                    let mut owned = String::new();
                    owned.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
                    owned.push_str(text);
                    Some(Value::Text(owned))
                } else { None }
            }
            4 => {
                let n = usize::try_from(self.arg(ai)?).map_err(|_| Reject::Range)?;
                if n > self.bytes.len() - self.pos || n > self.limits.nodes - self.usage.nodes {
                    return Err(Reject::Budget);
                }
                charge(&mut self.usage.elements, n, self.limits.elements)?;
                charge(&mut self.usage.expanded, n.checked_mul(8).ok_or(Reject::Budget)?,
                       self.limits.expanded)?;
                if let Some(Shape::List(child)) = shape {
                    // Charge element writes and all minimum child-node costs
                    // before allocation; recursive calls charge their own nodes.
                    charge(&mut self.usage.work, n, self.limits.work)?;
                    let min_children = n.checked_mul(32).ok_or(Reject::Budget)?;
                    if min_children > self.limits.expanded - self.usage.expanded {
                        return Err(Reject::Budget);
                    }
                    let mut values = Vec::new();
                    values.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
                    for _ in 0..n {
                        values.push(self.item(depth + 1, Some(child))?.ok_or(Reject::Shape)?);
                    }
                    Some(Value::List(values))
                } else {
                    for _ in 0..n { let _ = self.item(depth + 1, None)?; }
                    None
                }
            }
            // Fixed binary64 profile, not RFC preferred shortest float encoding.
            // Reject binary16/32, null, undefined and every simple value but bool.
            7 => match ai {
                20 | 21 => shape.map(|_| Value::Bool(ai == 21)),
                27 => {
                    let b: [u8; 8] = self.take(8)?.try_into().map_err(|_| Reject::Truncated)?;
                    let v = f64::from_bits(u64::from_be_bytes(b));
                    if !v.is_finite() { return Err(Reject::Range); }
                    shape.map(|_| Value::F64(v))
                }
                _ => return Err(Reject::Unsupported),
            },
            _ => return Err(Reject::Unsupported),
        };
        Ok(value)
    }
}

pub fn preflight(bytes: &[u8], limits: Limits) -> Result<Usage, Reject> {
    parse(bytes, limits, None).map(|(_, usage)| usage)
}

/// Single pass, immutable input, pinned caller-owned descriptor; charges
/// materialization in addition to parsing. Caller still needs tenant and
/// receiver heap credit, authenticated codec binding and atomic publication.
pub fn decode_typed(bytes: &[u8], limits: Limits, shape: &Shape<'_>) -> Result<(Value, Usage), Reject> {
    let (value, usage) = parse(bytes, limits, Some(shape))?;
    Ok((value.ok_or(Reject::Shape)?, usage))
}

fn parse(bytes: &[u8], limits: Limits, shape: Option<&Shape<'_>>) -> Result<(Option<Value>, Usage), Reject> {
    if limits.wire == 0 || limits.wire > 1024*1024 ||
       limits.expanded == 0 || limits.expanded > 8*1024*1024 ||
       limits.nodes == 0 || limits.nodes > 100000 ||
       limits.elements == 0 || limits.elements > 100000 ||
       limits.work == 0 || limits.work > 4*1024*1024 || limits.depth > 64 {
        return Err(Reject::Limits);
    }
    if bytes.len() > limits.wire { return Err(Reject::Budget); }
    let mut r = Reader { bytes, pos: 0, limits, usage: Usage::default() };
    let value = r.item(0, shape)?;
    if r.pos != bytes.len() { return Err(Reject::Trailing); }
    Ok((value, r.usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> Limits {
        Limits { wire: 1024, expanded: 8192, nodes: 100, elements: 100, work: 4096, depth: 8 }
    }
    #[test] fn typed_nested_and_exact_shared_budget() {
        let leaf = Shape::Text;
        let inner = Shape::List(&leaf);
        let shape = Shape::List(&inner);
        let bytes = [0x81,0x82,0x61,b'a',0x61,b'b'];
        let (v,u) = decode_typed(&bytes,limits(),&shape).unwrap();
        assert_eq!(v,Value::List(vec![Value::List(vec![Value::Text("a".into()),Value::Text("b".into())])]));
        assert_eq!(u,Usage { expanded:160,nodes:4,elements:3,work:17 });
        let exact=Limits{wire:6,expanded:160,nodes:4,elements:3,work:17,depth:2};
        assert_eq!(decode_typed(&bytes,exact,&shape),Ok((v,u)));
        assert_eq!(decode_typed(&bytes,Limits{work:16,..exact},&shape),Err(Reject::Budget));
        assert_eq!(decode_typed(&bytes,Limits{expanded:159,..exact},&shape),Err(Reject::Budget));
        assert_eq!(decode_typed(&bytes,limits(),&Shape::Bool),Err(Reject::Shape));
        assert_eq!(decode_typed(&[0x81,0x81,0xf5],limits(),&shape),Err(Reject::Shape));
    }
    #[test] fn typed_scalar_and_no_partial_value() {
        assert_eq!(decode_typed(&[0xf5],limits(),&Shape::Bool).unwrap().0,Value::Bool(true));
        assert_eq!(decode_typed(&[0x20],limits(),&Shape::I64).unwrap().0,Value::I64(-1));
        let mut b=[0;9]; b[0]=0xfb; b[1..].copy_from_slice(&1.5f64.to_bits().to_be_bytes());
        assert_eq!(decode_typed(&b,limits(),&Shape::F64).unwrap().0,Value::F64(1.5));
        let list=Shape::List(&Shape::Text);
        assert_eq!(decode_typed(&[0x82,0x61,b'a',0x61,0xff],limits(),&list),Err(Reject::Utf8));
        assert_eq!(decode_typed(&[0x61,b'x',0xf4],limits(),&Shape::Text),Err(Reject::Trailing));
    }
    #[test] fn exact_nested_budget() {
        // [true, ["hi", -1]]
        let b = [0x82,0xf5,0x82,0x62,b'h',b'i',0x20];
        let u = preflight(&b, limits()).unwrap();
        assert_eq!(u, Usage { expanded: 200, nodes: 5, elements: 4, work: 14 });
        let exact = Limits { wire: 7, expanded: 200, nodes: 5, elements: 4, work: 14, depth: 2 };
        assert_eq!(preflight(&b, exact), Ok(u));
        for l in [Limits{wire:6,..exact}, Limits{expanded:199,..exact},
                  Limits{nodes:4,..exact}, Limits{elements:3,..exact},
                  Limits{work:13,..exact}, Limits{depth:1,..exact}] {
            assert_eq!(preflight(&b,l), Err(Reject::Budget));
        }
    }
    #[test] fn signed_i64_boundaries() {
        for head in [0x1b,0x3b] {
            let mut b = [0xff;9]; b[0]=head; b[1]=0x7f;
            assert!(preflight(&b,limits()).is_ok());
            b[1]=0x80;
            assert_eq!(preflight(&b,limits()),Err(Reject::Range));
        }
    }
    #[test] fn reject_noncanonical_and_unsupported() {
        for b in [&[0x18,0x17][..], &[0x78,0x01,b'x'], &[0x98,0x00]] {
            assert_eq!(preflight(b,limits()),Err(Reject::NonCanonical));
        }
        for b in [&[0x9f,0xff][..], &[0xa0], &[0xc0,0x00], &[0x40], &[0xf6], &[0xf9,0,0]] {
            assert_eq!(preflight(b,limits()),Err(Reject::Unsupported));
        }
    }
    #[test] fn floats_utf8_and_trailing() {
        let mut b=[0;9]; b[0]=0xfb;
        b[1..].copy_from_slice(&1.5f64.to_bits().to_be_bytes());
        assert!(preflight(&b,limits()).is_ok());
        b[1..].copy_from_slice(&f64::INFINITY.to_bits().to_be_bytes());
        assert_eq!(preflight(&b,limits()),Err(Reject::Range));
        assert_eq!(preflight(&[0x61,0xff],limits()),Err(Reject::Utf8));
        assert_eq!(preflight(&[0x62,b'x'],limits()),Err(Reject::Truncated));
        assert_eq!(preflight(&[0xf4,0xf5],limits()),Err(Reject::Trailing));
    }
    #[test] fn huge_lengths_and_depth() {
        assert!(matches!(preflight(&[0x9b,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff],limits()),Err(Reject::Budget | Reject::Range)));
        let b=[0x81,0x81,0x81,0xf4];
        assert_eq!(preflight(&b,Limits{depth:2,..limits()}),Err(Reject::Budget));
        assert_eq!(preflight(&[0xf4],Limits{depth:65,..limits()}),Err(Reject::Limits));
    }
    #[test] fn bounded_mutation_smoke() {
        let mut state=0x12345678u32;
        for _ in 0..4096 {
            let mut b=[0u8;32];
            for x in &mut b {
                state=state.wrapping_mul(1664525).wrapping_add(1013904223);
                *x=(state>>24) as u8;
            }
            let _=preflight(&b,limits());
        }
    }
}
