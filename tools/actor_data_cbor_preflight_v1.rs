//! Host-only deterministic actor DATA CBOR subset preflight.
//! Preflight is allocation-free; typed decode allocates only after charging.
//! No descriptor inference, no maps/tags/indefinite items.
//! Includes opt-in typed decoding and bounded deterministic encoding.
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
/// Record fields are in authenticated schema order and encoded as definite
/// arrays. The codec does not itself authenticate nominal schema identity.
#[derive(Clone, Copy)]
pub enum Shape<'a> { Bool, I64, F64, Text, List(&'a Shape<'a>), Record(&'a [Shape<'a>]) }

#[derive(Debug, PartialEq)]
pub enum Value { Bool(bool), I64(i64), F64(f64), Text(String), List(Vec<Value>), Record(Vec<Value>) }

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
                Shape::List(_) | Shape::Record(_) => major == 4,
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
                // Registered record layout is positional and fixed-arity; no
                // untrusted key, field name, or descriptor selection on the wire.
                if let Some(Shape::Record(fields)) = shape {
                    if n != fields.len() { return Err(Reject::Shape); }
                }
                if n > self.bytes.len() - self.pos || n > self.limits.nodes - self.usage.nodes {
                    return Err(Reject::Budget);
                }
                charge(&mut self.usage.elements, n, self.limits.elements)?;
                charge(&mut self.usage.expanded, n.checked_mul(8).ok_or(Reject::Budget)?,
                       self.limits.expanded)?;
                if let Some(expected) = shape {
                    // Charge element writes and all minimum child-node costs
                    // before allocation; recursive calls charge their own nodes.
                    charge(&mut self.usage.work, n, self.limits.work)?;
                    let min_children = n.checked_mul(32).ok_or(Reject::Budget)?;
                    // Every child needs at least one visit and one head-byte
                    // read. Check this lower bound without double charging:
                    // the recursive calls account for the actual work.
                    let min_child_work = n.checked_mul(2).ok_or(Reject::Budget)?;
                    if min_children > self.limits.expanded - self.usage.expanded ||
                       min_child_work > self.limits.work - self.usage.work ||
                       (n != 0 && depth == self.limits.depth) {
                        return Err(Reject::Budget);
                    }
                    let mut values = Vec::new();
                    values.try_reserve_exact(n).map_err(|_| Reject::Allocation)?;
                    for index in 0..n {
                        let child = match expected {
                            Shape::List(child) => *child,
                            Shape::Record(fields) => fields.get(index).ok_or(Reject::Shape)?,
                            _ => return Err(Reject::Shape),
                        };
                        values.push(self.item(depth + 1, Some(child))?.ok_or(Reject::Shape)?);
                    }
                    Some(match expected {
                        Shape::List(_) => Value::List(values),
                        Shape::Record(_) => Value::Record(values),
                        _ => return Err(Reject::Shape),
                    })
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
    validate_limits(limits)?;
    if bytes.len() > limits.wire { return Err(Reject::Budget); }
    let mut r = Reader { bytes, pos: 0, limits, usage: Usage::default() };
    let value = r.item(0, shape)?;
    if r.pos != bytes.len() { return Err(Reject::Trailing); }
    Ok((value, r.usage))
}

fn validate_limits(limits: Limits) -> Result<(), Reject> {
    if limits.wire == 0 || limits.wire > 1024*1024 ||
       limits.expanded == 0 || limits.expanded > 8*1024*1024 ||
       limits.nodes == 0 || limits.nodes > 100000 ||
       limits.elements == 0 || limits.elements > 100000 ||
       limits.work == 0 || limits.work > 4*1024*1024 || limits.depth > 64 {
        return Err(Reject::Limits);
    }
    Ok(())
}

/// Bounded deterministic encoder for the same trusted positional profile.
/// Measures without allocating, then reserves the exact wire length once.
/// Work is cumulative across both passes; expanded includes output bytes.
/// This is a logical budget, not a physical allocator/receiver heap guarantee.
pub fn encode_typed(value: &Value, shape: &Shape<'_>, limits: Limits) -> Result<(Vec<u8>, Usage), Reject> {
    validate_limits(limits)?;
    let mut e = Encoder { limits, usage: Usage::default(), pos: 0, output: None };
    e.item(value, shape, 0)?;
    let size = e.pos;
    charge(&mut e.usage.expanded, size, limits.expanded)?;
    // Both passes visit precisely the same immutable value and descriptor.
    // Reject an insufficient total work budget before allocating output.
    if e.usage.work > limits.work - e.usage.work { return Err(Reject::Budget); }
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| Reject::Allocation)?;
    e.output = Some(out);
    e.pos = 0;
    e.item(value, shape, 0)?;
    if e.pos != size { return Err(Reject::Range); }
    Ok((e.output.take().ok_or(Reject::Allocation)?, e.usage))
}

struct Encoder {
    limits: Limits,
    usage: Usage,
    pos: usize,
    output: Option<Vec<u8>>,
}

impl Encoder {
    fn emit(&mut self, bytes: &[u8]) -> Result<(), Reject> {
        charge(&mut self.pos, bytes.len(), self.limits.wire)?;
        charge(&mut self.usage.work, bytes.len(), self.limits.work)?;
        if let Some(out) = &mut self.output {
            if self.pos > out.capacity() { return Err(Reject::Allocation); }
            out.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn arg(&mut self, major: u8, v: u64) -> Result<(), Reject> {
        let head = major << 5;
        match v {
            0..=23 => self.emit(&[head | v as u8]),
            24..=255 => self.emit(&[head | 24, v as u8]),
            256..=65535 => {
                self.emit(&[head | 25])?;
                self.emit(&(v as u16).to_be_bytes())
            }
            65536..=4294967295 => {
                self.emit(&[head | 26])?;
                self.emit(&(v as u32).to_be_bytes())
            }
            _ => {
                self.emit(&[head | 27])?;
                self.emit(&v.to_be_bytes())
            }
        }
    }

    fn item(&mut self, value: &Value, shape: &Shape<'_>, depth: usize) -> Result<(), Reject> {
        if depth > self.limits.depth { return Err(Reject::Budget); }
        let measuring = self.output.is_none();
        charge(&mut self.usage.work, 1, self.limits.work)?;
        if measuring {
            charge(&mut self.usage.nodes, 1, self.limits.nodes)?;
            charge(&mut self.usage.expanded, 32, self.limits.expanded)?;
        }
        match (value, shape) {
            (Value::Bool(v), Shape::Bool) => self.emit(&[if *v { 0xf5 } else { 0xf4 }]),
            (Value::I64(v), Shape::I64) => {
                if *v >= 0 { self.arg(0, *v as u64) }
                else { self.arg(1, (-1 - *v) as u64) }
            }
            (Value::F64(v), Shape::F64) => {
                if !v.is_finite() { return Err(Reject::Range); }
                self.emit(&[0xfb])?;
                self.emit(&v.to_bits().to_be_bytes())
            }
            (Value::Text(v), Shape::Text) => {
                if measuring {
                    charge(&mut self.usage.expanded, v.len().checked_mul(4).ok_or(Reject::Budget)?, self.limits.expanded)?;
                }
                self.arg(3, u64::try_from(v.len()).map_err(|_| Reject::Range)?)?;
                self.emit(v.as_bytes())
            }
            (Value::Record(items), Shape::Record(fields)) => {
                if items.len() != fields.len() { return Err(Reject::Shape); }
                if measuring {
                    charge(&mut self.usage.elements, items.len(), self.limits.elements)?;
                    charge(&mut self.usage.expanded, items.len().checked_mul(8).ok_or(Reject::Budget)?, self.limits.expanded)?;
                }
                self.arg(4, u64::try_from(items.len()).map_err(|_| Reject::Range)?)?;
                for (item, field) in items.iter().zip(fields.iter()) {
                    self.item(item, field, depth + 1)?;
                }
                Ok(())
            }
            (Value::List(items), Shape::List(child)) => {
                if measuring {
                    charge(&mut self.usage.elements, items.len(), self.limits.elements)?;
                    charge(&mut self.usage.expanded, items.len().checked_mul(8).ok_or(Reject::Budget)?, self.limits.expanded)?;
                }
                self.arg(4, u64::try_from(items.len()).map_err(|_| Reject::Range)?)?;
                for item in items { self.item(item, child, depth + 1)?; }
                Ok(())
            }
            _ => Err(Reject::Shape),
        }
    }
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
    #[test] fn typed_list_reservation_requires_minimum_child_credits() {
        let shape = Shape::List(&Shape::Bool);
        let bytes = [0x82, 0xf5, 0xf4];
        let exact = Limits { wire: 3, expanded: 112, nodes: 3, elements: 2, work: 8, depth: 1 };
        let (_, usage) = decode_typed(&bytes, exact, &shape).unwrap();
        assert_eq!(usage, Usage { expanded: 112, nodes: 3, elements: 2, work: 8 });
        assert_eq!(decode_typed(&bytes, Limits { work: 7, ..exact }, &shape), Err(Reject::Budget));
        assert_eq!(decode_typed(&bytes, Limits { depth: 0, ..exact }, &shape), Err(Reject::Budget));
        // When even the minimum child work cannot fit, fail on the budget
        // before inspecting a malformed child or reserving the list.
        assert_eq!(decode_typed(&[0x82, 0xf6, 0xf4], Limits { work: 7, ..exact }, &shape), Err(Reject::Budget));
        assert_eq!(decode_typed(&[0x82, 0xf6, 0xf4], exact, &shape), Err(Reject::Shape));
        // Empty lists require no child depth or work.
        let empty = Limits { wire: 1, expanded: 32, nodes: 1, elements: 1, work: 2, depth: 0 };
        assert_eq!(decode_typed(&[0x80], empty, &shape), Ok((Value::List(vec![]), Usage { expanded: 32, nodes: 1, elements: 0, work: 2 })));
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

    #[test] fn encode_nested_exact_cumulative_budget() {
        let leaf = Shape::Text;
        let inner = Shape::List(&leaf);
        let shape = Shape::List(&inner);
        let v = Value::List(vec![Value::List(vec![Value::Text("a".into()),Value::Text("b".into())])]);
        let exact = Limits {wire:6,expanded:166,nodes:4,elements:3,work:20,depth:2};
        let (b,u) = encode_typed(&v,&shape,exact).unwrap();
        assert_eq!(b,[0x81,0x82,0x61,b'a',0x61,b'b']);
        assert_eq!(u,Usage{expanded:166,nodes:4,elements:3,work:20});
        assert_eq!(decode_typed(&b,exact,&shape).unwrap().0,v);
        for l in [Limits{wire:5,..exact}, Limits{expanded:165,..exact},
                  Limits{nodes:3,..exact}, Limits{elements:2,..exact},
                  Limits{work:19,..exact}, Limits{depth:1,..exact}] {
            assert_eq!(encode_typed(&v,&shape,l),Err(Reject::Budget));
        }
    }
    #[test] fn encode_canonical_signed_boundaries() {
        for n in [i64::MIN,-4294967297,-65537,-257,-25,-24,-1,0,23,24,255,256,65535,65536,4294967295,4294967296,i64::MAX] {
            let v=Value::I64(n);
            let (b,_)=encode_typed(&v,&Shape::I64,limits()).unwrap();
            assert_eq!(decode_typed(&b,limits(),&Shape::I64).unwrap().0,v);
            assert!(preflight(&b,limits()).is_ok());
        }
        assert_eq!(encode_typed(&Value::I64(i64::MIN),&Shape::I64,limits()).unwrap().0,[0x3b,0x7f,0xff,0xff,0xff,0xff,0xff,0xff,0xff]);
        assert_eq!(encode_typed(&Value::I64(24),&Shape::I64,limits()).unwrap().0,[0x18,24]);
    }
    #[test] fn encode_text_float_and_empty() {
        let v=Value::Text("é🙂".into());
        let (b,_)=encode_typed(&v,&Shape::Text,limits()).unwrap();
        assert_eq!(b[0],0x66);
        assert_eq!(decode_typed(&b,limits(),&Shape::Text).unwrap().0,v);
        for n in [-0.0f64,0.0,1.5,f64::MIN,f64::MAX] {
            let (b,_)=encode_typed(&Value::F64(n),&Shape::F64,limits()).unwrap();
            assert_eq!(b[0],0xfb);
            assert_eq!(b[1..],n.to_bits().to_be_bytes());
        }
        assert_eq!(encode_typed(&Value::List(vec![]),&Shape::List(&Shape::Bool),limits()).unwrap().0,[0x80]);
        assert_eq!(encode_typed(&Value::Bool(false),&Shape::Bool,limits()).unwrap().0,[0xf4]);
    }
    #[test] fn encode_refuses_shape_range_and_limits() {
        let v=Value::List(vec![Value::Bool(true),Value::I64(1)]);
        assert_eq!(encode_typed(&v,&Shape::List(&Shape::Bool),limits()),Err(Reject::Shape));
        for n in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY] {
            assert_eq!(encode_typed(&Value::F64(n),&Shape::F64,limits()),Err(Reject::Range));
        }
        assert_eq!(encode_typed(&Value::Bool(true),&Shape::Bool,Limits{depth:65,..limits()}),Err(Reject::Limits));
    }

    #[test] fn record_exact_budgets_and_wire() {
        let fields = [Shape::Bool, Shape::Text];
        let shape = Shape::Record(&fields);
        let v = Value::Record(vec![Value::Bool(true), Value::Text("x".into())]);
        let bytes = [0x82, 0xf5, 0x61, b'x'];
        let decode = Limits {wire:4,expanded:116,nodes:3,elements:2,work:11,depth:1};
        let (decoded, usage) = decode_typed(&bytes, decode, &shape).unwrap();
        assert_eq!(decoded,v);
        assert_eq!(usage,Usage{expanded:116,nodes:3,elements:2,work:11});
        for l in [Limits{wire:3,..decode},Limits{expanded:115,..decode},
                  Limits{nodes:2,..decode},Limits{elements:1,..decode},
                  Limits{work:10,..decode},Limits{depth:0,..decode}] {
            assert_eq!(decode_typed(&bytes,l,&shape),Err(Reject::Budget));
        }
        let encode = Limits{expanded:120,work:14,..decode};
        assert_eq!(encode_typed(&v,&shape,encode),Ok((bytes.to_vec(),Usage{expanded:120,nodes:3,elements:2,work:14})));
        for l in [Limits{wire:3,..encode},Limits{expanded:119,..encode},
                  Limits{nodes:2,..encode},Limits{elements:1,..encode},
                  Limits{work:13,..encode},Limits{depth:0,..encode}] {
            assert_eq!(encode_typed(&v,&shape,l),Err(Reject::Budget));
        }
        assert!(preflight(&bytes,limits()).is_ok());
    }

    #[test] fn record_rejects_arity_types_and_noncanonical_wire() {
        let fields = [Shape::Bool, Shape::I64];
        let shape = Shape::Record(&fields);
        for b in [&[0x81,0xf5][..], &[0x83,0xf5,0x01,0x02], &[0x82,0x01,0xf5], &[0xa0]] {
            assert_eq!(decode_typed(b,limits(),&shape),Err(Reject::Shape));
        }
        assert_eq!(decode_typed(&[0x98,0x02,0xf5,0x01],limits(),&shape),Err(Reject::NonCanonical));
        assert_eq!(decode_typed(&[0x82,0xf5,0x18,0x01],limits(),&shape),Err(Reject::NonCanonical));
        assert_eq!(decode_typed(&[0x82,0xf5,0x01,0x00],limits(),&shape),Err(Reject::Trailing));
        for v in [Value::Record(vec![Value::Bool(true)]),
                  Value::Record(vec![Value::I64(1),Value::Bool(true)]),
                  Value::List(vec![Value::Bool(true),Value::I64(1)])] {
            assert_eq!(encode_typed(&v,&shape,limits()),Err(Reject::Shape));
        }
    }

    #[test] fn nested_records_lists_and_empty_records() {
        let fields = [Shape::I64, Shape::Text];
        let record = Shape::Record(&fields);
        let list = Shape::List(&record);
        let outer_fields = [Shape::Bool, list, Shape::Record(&[])];
        let shape = Shape::Record(&outer_fields);
        let v = Value::Record(vec![Value::Bool(false),Value::List(vec![
            Value::Record(vec![Value::I64(-1),Value::Text("é".into())])
        ]),Value::Record(vec![])]);
        let (b,_) = encode_typed(&v,&shape,limits()).unwrap();
        assert_eq!(b,[0x83,0xf4,0x81,0x82,0x20,0x62,0xc3,0xa9,0x80]);
        assert_eq!(decode_typed(&b,limits(),&shape).unwrap().0,v);
        assert_eq!(decode_typed(&b,Limits{depth:2,..limits()},&shape),Err(Reject::Budget));
        assert_eq!(decode_typed(&[0x80],limits(),&Shape::Record(&[])).unwrap().0,Value::Record(vec![]));
        assert_eq!(encode_typed(&Value::Record(vec![]),&Shape::Record(&[]),limits()).unwrap().0,[0x80]);
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
