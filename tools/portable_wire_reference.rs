//! Bounded HOST-ONLY reference for tagged JSON, a deterministic subset of
//! MessagePack, and a candidate schema-bound ATB1 frame. Not a guest codec,
//! ratified Atto ABI, general schema engine, or proof of runtime admission.
use serde_json::{Map, Number, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

pub const MAX_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 32;
const MAX_ITEMS: usize = 4096;
const HEADER: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefError;
type R<T> = Result<T, RefError>;

fn spend(left: &mut usize, n: usize) -> R<()> {
    *left = left.checked_sub(n).ok_or(RefError)?;
    Ok(())
}
fn bounded_text(s: &str) -> R<()> {
    if s.len() > MAX_BYTES { return Err(RefError); }
    Ok(())
}
fn check_json(v: &Json, depth: usize, left: &mut usize) -> R<()> {
    if depth > MAX_DEPTH { return Err(RefError); }
    match v {
        Json::String(s) => bounded_text(s)?,
        Json::Array(a) => {
            spend(left, a.len())?;
            for child in a { check_json(child, depth+1, left)?; }
        }
        Json::Object(o) => {
            spend(left, o.len())?;
            for (key, child) in o {
                bounded_text(key)?;
                check_json(child, depth+1, left)?;
            }
        }
        Json::Number(n) => {
            let raw=n.to_string();
            if !raw.contains(['.', 'e', 'E']) {
                if raw.parse::<i64>().is_err() && raw.parse::<u64>().is_err() {
                    return Err(RefError);
                }
            } else if !n.as_f64().is_some_and(f64::is_finite) {
                return Err(RefError);
            }
        }
        _ => {}
    }
    Ok(())
}
fn valid_code(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && b.len() <= 64 && b[0].is_ascii_lowercase() &&
        b.iter().all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || *x == b'_')
}
fn exact_fields(o: &Map<String,Json>, keys: &[&str]) -> bool {
    o.len()==keys.len() && keys.iter().all(|k| o.contains_key(*k))
}
pub fn validate_tagged(v: &Json) -> R<()> {
    let o=v.as_object().ok_or(RefError)?;
    let kind=o.get("kind").and_then(Json::as_str).ok_or(RefError)?;
    let variant=o.get("variant").and_then(Json::as_str).ok_or(RefError)?;
    match (kind,variant) {
        ("option","none") if exact_fields(o,&["kind","variant"]) => Ok(()),
        ("option","some") | ("result","ok") if exact_fields(o,&["kind","variant","value"]) => {
            check_json(&o["value"],0,&mut MAX_ITEMS.clone())
        }
        ("result","err") if exact_fields(o,&["kind","variant","error"]) => {
            let e=o["error"].as_object().ok_or(RefError)?;
            if !exact_fields(e,&["code","message"]) ||
                !e["code"].as_str().is_some_and(valid_code) ||
                !e["message"].as_str().is_some_and(|s| s.len()<=512) {
                return Err(RefError);
            }
            Ok(())
        }
        _ => Err(RefError)
    }
}
// Do not use serde_json's default object materializer: duplicate members
// would already have been collapsed. Tokenize numbers before conversion so
// an out-of-range integral literal cannot silently become a float.
struct JParser<'a> { raw: &'a [u8], pos: usize, left: usize }
impl<'a> JParser<'a> {
    fn peek(&self)->Option<u8> { self.raw.get(self.pos).copied() }
    fn ws(&mut self) {
        while matches!(self.peek(),Some(b' '|b'\t'|b'\r'|b'\n')) { self.pos+=1; }
    }
    fn eat(&mut self,b:u8)->bool {
        if self.peek()==Some(b) { self.pos+=1; true } else { false }
    }
    fn require(&mut self,b:u8)->R<()> {
        if self.eat(b) { Ok(()) } else { Err(RefError) }
    }
    fn literal(&mut self,s:&[u8])->R<()> {
        if !self.raw[self.pos..].starts_with(s) { return Err(RefError); }
        self.pos+=s.len(); Ok(())
    }
    fn string(&mut self)->R<String> {
        let start=self.pos;
        self.require(b'"')?;
        loop {
            let b=self.peek().ok_or(RefError)?; self.pos+=1;
            match b {
                b'"' => {
                    return serde_json::from_slice::<String>(&self.raw[start..self.pos])
                        .map_err(|_|RefError);
                }
                b'\\' => { self.peek().ok_or(RefError)?; self.pos+=1; }
                0..=31 => return Err(RefError),
                _ => {}
            }
        }
    }
    fn digits(&mut self)->R<()> {
        let start=self.pos;
        while matches!(self.peek(),Some(b'0'..=b'9')) { self.pos+=1; }
        if start==self.pos { return Err(RefError); }
        Ok(())
    }
    fn number(&mut self)->R<Json> {
        let start=self.pos;
        let neg=self.eat(b'-');
        if !self.eat(b'0') { self.digits()?; }
        let mut fractional=false;
        if self.eat(b'.') { fractional=true; self.digits()?; }
        if self.eat(b'e') || self.eat(b'E') {
            fractional=true;
            if !self.eat(b'+') { self.eat(b'-'); }
            self.digits()?;
        }
        let s=std::str::from_utf8(&self.raw[start..self.pos]).map_err(|_|RefError)?;
        let n=if fractional {
            Number::from_f64(s.parse::<f64>().map_err(|_|RefError)?).ok_or(RefError)?
        } else if neg {
            Number::from(s.parse::<i64>().map_err(|_|RefError)?)
        } else {
            Number::from(s.parse::<u64>().map_err(|_|RefError)?)
        };
        Ok(Json::Number(n))
    }
    fn value(&mut self,depth:usize)->R<Json> {
        // The tagged wrapper itself is not part of the payload depth budget.
        if depth>MAX_DEPTH+2 { return Err(RefError); }
        self.ws();
        match self.peek().ok_or(RefError)? {
            b'n'=>{ self.literal(b"null")?; Ok(Json::Null) }
            b't'=>{ self.literal(b"true")?; Ok(Json::Bool(true)) }
            b'f'=>{ self.literal(b"false")?; Ok(Json::Bool(false)) }
            b'"'=>Ok(Json::String(self.string()?)),
            b'-'|b'0'..=b'9'=>self.number(),
            b'['=>{
                self.pos+=1; self.ws();
                let mut a=Vec::new();
                if self.eat(b']') { return Ok(Json::Array(a)); }
                loop {
                    spend(&mut self.left,1)?;
                    if a.len()>=MAX_ITEMS { return Err(RefError); }
                    a.push(self.value(depth+1)?);
                    self.ws();
                    if self.eat(b']') { break; }
                    self.require(b',')?;
                }
                Ok(Json::Array(a))
            }
            b'{'=>{
                self.pos+=1; self.ws();
                let mut o=Map::new();
                if self.eat(b'}') { return Ok(Json::Object(o)); }
                loop {
                    self.ws();
                    spend(&mut self.left,1)?;
                    if o.len()>=MAX_ITEMS { return Err(RefError); }
                    let key=self.string()?;
                    if o.contains_key(&key) { return Err(RefError); }
                    self.ws(); self.require(b':')?;
                    let value=self.value(depth+1)?;
                    o.insert(key,value);
                    self.ws();
                    if self.eat(b'}') { break; }
                    self.require(b',')?;
                }
                Ok(Json::Object(o))
            }
            _=>Err(RefError)
        }
    }
}
pub fn decode_tagged(raw:&[u8])->R<Json> {
    if raw.len()>MAX_BYTES { return Err(RefError); }
    let mut p=JParser{raw,pos:0,left:MAX_ITEMS+8};
    let v=p.value(0)?;
    p.ws();
    if p.pos!=raw.len() { return Err(RefError); }
    validate_tagged(&v)?;
    Ok(v)
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self,b:&[u8])->io::Result<usize> {
        if b.len()>MAX_BYTES-self.0.len() {
            return Err(io::Error::other("wire byte limit"));
        }
        self.0.extend_from_slice(b); Ok(b.len())
    }
    fn flush(&mut self)->io::Result<()> { Ok(()) }
}
fn write_json(v:&Json,out:&mut Bounded)->R<()> {
    match v {
        Json::Object(o)=>{
            out.write_all(b"{").map_err(|_|RefError)?;
            let mut keys:Vec<_>=o.keys().collect();
            keys.sort_unstable();
            for (i,key) in keys.into_iter().enumerate() {
                if i!=0 { out.write_all(b",").map_err(|_|RefError)?; }
                serde_json::to_writer(&mut *out,key).map_err(|_|RefError)?;
                out.write_all(b":").map_err(|_|RefError)?;
                write_json(&o[key],out)?;
            }
            out.write_all(b"}").map_err(|_|RefError)
        }
        Json::Array(a)=>{
            out.write_all(b"[").map_err(|_|RefError)?;
            for (i,value) in a.iter().enumerate() {
                if i!=0 { out.write_all(b",").map_err(|_|RefError)?; }
                write_json(value,out)?;
            }
            out.write_all(b"]").map_err(|_|RefError)
        }
        _=>serde_json::to_writer(&mut *out,v).map_err(|_|RefError)
    }
}
pub fn encode_tagged(v:&Json)->R<Vec<u8>> {
    validate_tagged(v)?;
    let mut out=Bounded(Vec::new());
    write_json(v,&mut out)?;
    Ok(out.0)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Wire {
    Null, Bool(bool), Int(i128), Float(f64), Text(String), Binary(Vec<u8>),
    Array(Vec<Wire>), Map(BTreeMap<String,Wire>),
}
fn emit(out:&mut Vec<u8>,bytes:&[u8])->R<()> {
    if bytes.len()>MAX_BYTES-out.len() { return Err(RefError); }
    out.extend_from_slice(bytes); Ok(())
}
fn length(out:&mut Vec<u8>,n:usize,fix:Option<(usize,u8)>,tag8:Option<u8>,tag16:u8,tag32:u8)->R<()> {
    if n>MAX_BYTES { return Err(RefError); }
    if let Some((cap,base))=fix {
        if n<cap { return emit(out,&[base|n as u8]); }
    }
    if let Some(tag)=tag8 {
        if n<=u8::MAX as usize { return emit(out,&[tag,n as u8]); }
    }
    if n<=u16::MAX as usize {
        emit(out,&[tag16])?;
        emit(out,&(n as u16).to_be_bytes())
    } else {
        emit(out,&[tag32])?;
        emit(out,&(n as u32).to_be_bytes())
    }
}
fn text_wire(out:&mut Vec<u8>,s:&str)->R<()> {
    length(out,s.len(),Some((32,0xa0)),Some(0xd9),0xda,0xdb)?;
    emit(out,s.as_bytes())
}
fn number_wire(out:&mut Vec<u8>,v:i128)->R<()> {
    if v<i64::MIN as i128 || v>u64::MAX as i128 { return Err(RefError); }
    if (0..=127).contains(&v) || (-32..=-1).contains(&v) {
        return emit(out,&[v as u8]);
    }
    if v>=0 {
        let v=v as u64;
        if v<=u8::MAX as u64 { emit(out,&[0xcc,v as u8]) }
        else if v<=u16::MAX as u64 { emit(out,&[0xcd])?; emit(out,&(v as u16).to_be_bytes()) }
        else if v<=u32::MAX as u64 { emit(out,&[0xce])?; emit(out,&(v as u32).to_be_bytes()) }
        else { emit(out,&[0xcf])?; emit(out,&v.to_be_bytes()) }
    } else {
        let v=v as i64;
        if v>=i8::MIN as i64 { emit(out,&[0xd0,v as u8]) }
        else if v>=i16::MIN as i64 { emit(out,&[0xd1])?; emit(out,&(v as i16).to_be_bytes()) }
        else if v>=i32::MIN as i64 { emit(out,&[0xd2])?; emit(out,&(v as i32).to_be_bytes()) }
        else { emit(out,&[0xd3])?; emit(out,&v.to_be_bytes()) }
    }
}
fn encode_wire(v:&Wire,out:&mut Vec<u8>,depth:usize,left:&mut usize)->R<()> {
    if depth>MAX_DEPTH { return Err(RefError); }
    match v {
        Wire::Null=>emit(out,&[0xc0]),
        Wire::Bool(b)=>emit(out,&[if *b {0xc3} else {0xc2}]),
        Wire::Int(n)=>number_wire(out,*n),
        Wire::Float(n)=>{
            if !n.is_finite() { return Err(RefError); }
            emit(out,&[0xcb])?; emit(out,&n.to_bits().to_be_bytes())
        }
        Wire::Text(s)=>text_wire(out,s),
        Wire::Binary(b)=>{
            length(out,b.len(),None,Some(0xc4),0xc5,0xc6)?;
            emit(out,b)
        }
        Wire::Array(a)=>{
            spend(left,a.len())?;
            length(out,a.len(),Some((16,0x90)),None,0xdc,0xdd)?;
            for child in a { encode_wire(child,out,depth+1,left)?; }
            Ok(())
        }
        Wire::Map(m)=>{
            spend(left,m.len())?;
            length(out,m.len(),Some((16,0x80)),None,0xde,0xdf)?;
            if !m.is_empty() && depth==MAX_DEPTH { return Err(RefError); }
            for (k,child) in m {
                text_wire(out,k)?;
                encode_wire(child,out,depth+1,left)?;
            }
            Ok(())
        }
    }
}
pub fn encode_msgpack(v:&Wire)->R<Vec<u8>> {
    let mut out=Vec::new();
    encode_wire(v,&mut out,0,&mut MAX_ITEMS.clone())?;
    Ok(out)
}
struct Reader<'a> { raw:&'a [u8], pos:usize, left:usize }
impl<'a> Reader<'a> {
    fn take(&mut self,n:usize)->R<&'a [u8]> {
        let end=self.pos.checked_add(n).ok_or(RefError)?;
        let x=self.raw.get(self.pos..end).ok_or(RefError)?;
        self.pos=end; Ok(x)
    }
    fn bytes<const N:usize>(&mut self)->R<[u8;N]> {
        self.take(N)?.try_into().map_err(|_|RefError)
    }
    fn u8(&mut self)->R<u8> { Ok(self.bytes::<1>()?[0]) }
    fn u16(&mut self)->R<u16> { Ok(u16::from_be_bytes(self.bytes()?)) }
    fn u32(&mut self)->R<u32> { Ok(u32::from_be_bytes(self.bytes()?)) }
    fn u64(&mut self)->R<u64> { Ok(u64::from_be_bytes(self.bytes()?)) }
    fn len(&mut self,b:u8)->R<usize> {
        let n=match b {
            0xc4|0xd9=>u64::from(self.u8()?),
            0xc5|0xda|0xdc|0xde=>u64::from(self.u16()?),
            0xc6|0xdb|0xdd|0xdf=>u64::from(self.u32()?),
            _=>return Err(RefError),
        };
        if n>MAX_BYTES as u64 { return Err(RefError); }
        usize::try_from(n).map_err(|_|RefError)
    }
    fn text(&mut self,n:usize)->R<Wire> {
        let s=std::str::from_utf8(self.take(n)?).map_err(|_|RefError)?;
        Ok(Wire::Text(s.to_owned()))
    }
    fn seq(&mut self,n:usize,map:bool,depth:usize)->R<Wire> {
        spend(&mut self.left,n)?;
        let min=n.checked_mul(if map {2} else {1}).ok_or(RefError)?;
        if min>self.raw.len()-self.pos || (n!=0 && depth==MAX_DEPTH) { return Err(RefError); }
        if map {
            let mut m=BTreeMap::new();
            for _ in 0..n {
                let Wire::Text(k)=self.read(depth+1)? else { return Err(RefError); };
                if m.contains_key(&k) { return Err(RefError); }
                m.insert(k,self.read(depth+1)?);
            }
            Ok(Wire::Map(m))
        } else {
            let mut a=Vec::new();
            a.try_reserve_exact(n).map_err(|_|RefError)?;
            for _ in 0..n { a.push(self.read(depth+1)?); }
            Ok(Wire::Array(a))
        }
    }
    fn read(&mut self,depth:usize)->R<Wire> {
        if depth>MAX_DEPTH { return Err(RefError); }
        let b=self.u8()?;
        let v=match b {
            0x00..=0x7f=>Wire::Int(i128::from(b)),
            0xe0..=0xff=>Wire::Int(i128::from(b as i8)),
            0x80..=0x8f=>return self.seq((b&15) as usize,true,depth),
            0x90..=0x9f=>return self.seq((b&15) as usize,false,depth),
            0xa0..=0xbf=>return self.text((b&31) as usize),
            0xc0=>Wire::Null,
            0xc2=>Wire::Bool(false),
            0xc3=>Wire::Bool(true),
            0xc4..=0xc6=>{ let n=self.len(b)?; Wire::Binary(self.take(n)?.to_vec()) }
            0xca=>Wire::Float(f64::from(f32::from_bits(self.u32()?))),
            0xcb=>Wire::Float(f64::from_bits(self.u64()?)),
            0xcc=>Wire::Int(i128::from(self.u8()?)),
            0xcd=>Wire::Int(i128::from(self.u16()?)),
            0xce=>Wire::Int(i128::from(self.u32()?)),
            0xcf=>Wire::Int(i128::from(self.u64()?)),
            0xd0=>Wire::Int(i128::from(self.u8()? as i8)),
            0xd1=>Wire::Int(i128::from(self.u16()? as i16)),
            0xd2=>Wire::Int(i128::from(self.u32()? as i32)),
            0xd3=>Wire::Int(i128::from(self.u64()? as i64)),
            0xd9..=0xdb=>{ let n=self.len(b)?; return self.text(n); }
            0xdc..=0xdd=>{ let n=self.len(b)?; return self.seq(n,false,depth); }
            0xde..=0xdf=>{ let n=self.len(b)?; return self.seq(n,true,depth); }
            _=>return Err(RefError)
        };
        if matches!(&v,Wire::Float(n) if !n.is_finite()) { return Err(RefError); }
        Ok(v)
    }
}
pub fn decode_msgpack(raw:&[u8])->R<Wire> {
    if raw.len()>MAX_BYTES { return Err(RefError); }
    let mut r=Reader{raw,pos:0,left:MAX_ITEMS};
    let v=r.read(0)?;
    if r.pos!=raw.len() { return Err(RefError); }
    Ok(v)
}
pub fn validate_external_names(names:&BTreeMap<String,String>)->R<()> {
    let mut left=MAX_BYTES;
    if names.len()>MAX_ITEMS { return Err(RefError); }
    let mut seen=BTreeSet::new();
    for (internal,external) in names {
        spend(&mut left,internal.len())?;
        spend(&mut left,external.len())?;
        let b=internal.as_bytes();
        if b.is_empty() || !(b[0].is_ascii_alphabetic() || b[0]==b'_') ||
           !b.iter().all(|x|x.is_ascii_alphanumeric()||*x==b'_') {
            return Err(RefError);
        }
        if !seen.insert(external.as_str()) { return Err(RefError); }
    }
    Ok(())
}
pub fn encode_atto_binary(v:&Wire,schema:&[u8;32])->R<Vec<u8>> {
    if schema.iter().all(|b|*b==0) { return Err(RefError); }
    let payload=encode_msgpack(v)?;
    let mut out=Vec::with_capacity(HEADER+payload.len());
    out.extend_from_slice(b"ATB1\x01\x00\x00\x00");
    out.extend_from_slice(schema);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}
pub fn decode_atto_binary(raw:&[u8],schema:&[u8;32])->R<Wire> {
    if raw.len()<HEADER || raw.len()>HEADER+MAX_BYTES ||
       schema.iter().all(|b|*b==0) || &raw[..8]!=b"ATB1\x01\x00\x00\x00" ||
       &raw[8..40]!=schema { return Err(RefError); }
    let size=u32::from_be_bytes(raw[40..44].try_into().map_err(|_|RefError)?) as usize;
    if size!=raw.len()-HEADER { return Err(RefError); }
    let payload=&raw[HEADER..];
    let value=decode_msgpack(payload)?;
    if encode_msgpack(&value)?.as_slice()!=payload { return Err(RefError); }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const SCHEMA:[u8;32]=[3;32];
    #[test] fn tagged_round_trips() {
        for v in [
            json!({"kind":"option","variant":"none"}),
            json!({"kind":"option","variant":"some","value":null}),
            json!({"kind":"result","variant":"ok","value":{"ñ":[1,true]}}),
            json!({"kind":"result","variant":"err","error":{"code":"not_found","message":"ñ"}})
        ] { assert_eq!(decode_tagged(&encode_tagged(&v).unwrap()).unwrap(),v); }
    }
    #[test] fn tagged_deterministic_nested_keys_and_unicode() {
        let raw=r#"{"variant":"ok","value":{"z":1,"a":"\ud83d\ude00"},"kind":"result"}"#;
        let v=decode_tagged(raw.as_bytes()).unwrap();
        let canonical=r#"{"kind":"result","value":{"a":"😀","z":1},"variant":"ok"}"#;
        // Lexicographic order is explicit even when serde_json's preserve_order
        // feature is enabled by another dependency.
        assert_eq!(encode_tagged(&v).unwrap(),r#"{"kind":"result","value":{"a":"😀","z":1},"variant":"ok"}"#.as_bytes());
        assert_eq!(decode_tagged(canonical.as_bytes()).unwrap(),v);
    }
    #[test] fn tagged_fields_and_message_limits() {
        for v in [
            json!({"kind":"result","variant":"none"}),
            json!({"kind":"option","variant":"none","value":1}),
            json!({"kind":"result","variant":"err","error":{"code":"BAD","message":""}}),
            json!({"kind":"result","variant":"err","error":{"code":"bad","message":"é".repeat(257)}}),
            json!({"kind":"result","variant":"ok","value":1,"extra":0}),
        ] { assert_eq!(encode_tagged(&v),Err(RefError)); }
        let v=json!({"kind":"result","variant":"err","error":{"code":"bad","message":"é".repeat(256)}});
        assert_eq!(decode_tagged(&encode_tagged(&v).unwrap()).unwrap(),v);
    }
    #[test] fn tagged_raw_duplicate_and_surrogate_refusals() {
        for raw in [
            r#"{"kind":"option","kind":"option","variant":"none"}"#,
            r#"{"kind":"result","variant":"ok","value":{"a":1,"\u0061":2}}"#,
            r#"{"kind":"result","variant":"ok","value":"\ud800"}"#,
            r#"{"kind":"result","variant":"ok","value":{"\ud800":1}}"#,
            r#"{"kind":"result","variant":"err","error":{"code":"bad","message":"\udfff"}}"#,
            r#"{"kind":"option","variant":"none"} null"#,
        ] { assert_eq!(decode_tagged(raw.as_bytes()),Err(RefError)); }
        assert_eq!(decode_tagged(b"{\"kind\":\"result\",\"variant\":\"ok\",\"value\":\"\xff\"}"),Err(RefError));
    }
    #[test] fn tagged_numbers_exact_and_strict() {
        for s in ["18446744073709551616","-9223372036854775809","1e999","Infinity","NaN","01","1.","1e","--1"] {
            let raw=format!(r#"{{"kind":"result","variant":"ok","value":{s}}}"#);
            assert_eq!(decode_tagged(raw.as_bytes()),Err(RefError),"{s}");
        }
        for n in [json!(i64::MIN),json!(u64::MAX),json!(9007199254740993u64)] {
            let v=json!({"kind":"result","variant":"ok","value":n});
            assert_eq!(decode_tagged(&encode_tagged(&v).unwrap()).unwrap(),v);
        }
    }
    #[test] fn tagged_depth_items_and_output_bounds() {
        let mut v=Json::Null;
        for _ in 0..32 { v=json!([v]); }
        let tagged=json!({"kind":"result","variant":"ok","value":v.clone()});
        assert_eq!(decode_tagged(&encode_tagged(&tagged).unwrap()).unwrap(),tagged);
        assert!(encode_tagged(&json!({"kind":"result","variant":"ok","value":[v]})).is_err());
        let v=json!({"kind":"result","variant":"ok","value":vec![0;4096]});
        assert!(decode_tagged(&encode_tagged(&v).unwrap()).is_ok());
        assert!(encode_tagged(&json!({"kind":"result","variant":"ok","value":vec![0;4097]})).is_err());
        assert!(decode_tagged(&vec![b' ';MAX_BYTES+1]).is_err());
        assert!(encode_tagged(&json!({"kind":"result","variant":"ok","value":"x".repeat(MAX_BYTES)})).is_err());
    }
    fn map(v:i128)->Wire { Wire::Map(BTreeMap::from([("a".to_owned(),Wire::Int(v))])) }
    #[test] fn msgpack_golden_and_deterministic_map_order() {
        assert_eq!(encode_msgpack(&map(1)).unwrap(),[0x81,0xa1,b'a',1]);
        let a=Wire::Array(vec![Wire::Text("x".into()),Wire::Bool(true),Wire::Int(0)]);
        assert_eq!(encode_msgpack(&a).unwrap(),[0x93,0xa1,b'x',0xc3,0]);
        let m=Wire::Map(BTreeMap::from([("z".into(),Wire::Int(1)),("a".into(),Wire::Int(2))]));
        assert_eq!(encode_msgpack(&m).unwrap(),[0x82,0xa1,b'a',2,0xa1,b'z',1]);
    }
    #[test] fn msgpack_signed_unsigned_and_float_round_trips() {
        for n in [i64::MIN as i128,-4294967297,-65537,-32769,-129,-33,-32,-1,0,127,128,255,256,65535,65536,4294967295,4294967296,u64::MAX as i128] {
            let v=Wire::Int(n);
            assert_eq!(decode_msgpack(&encode_msgpack(&v).unwrap()).unwrap(),v);
        }
        for n in [0.0,-0.0,1.25,f64::MIN,f64::MAX] {
            let v=Wire::Float(n);
            let b=encode_msgpack(&v).unwrap();
            assert_eq!(b[0],0xcb);
            let Wire::Float(got)=decode_msgpack(&b).unwrap() else { panic!("not float"); };
            assert_eq!(got.to_bits(),n.to_bits());
        }
    }
    #[test] fn msgpack_text_binary_nested_and_empty() {
        for v in [
            Wire::Null,Wire::Bool(false),Wire::Text("ñ😀".into()),Wire::Text("".into()),
            Wire::Binary(vec![0,255]),Wire::Binary(vec![]),Wire::Array(vec![]),
            Wire::Map(BTreeMap::new()),Wire::Array(vec![Wire::Bool(true),map(8)])
        ] { assert_eq!(decode_msgpack(&encode_msgpack(&v).unwrap()).unwrap(),v); }
        assert_ne!(encode_msgpack(&Wire::Text("hello".into())),encode_msgpack(&Wire::Binary(b"hello".to_vec())));
    }
    #[test] fn msgpack_valid_noncanonical_decodes() {
        assert_eq!(decode_msgpack(&[0xcc,1]),Ok(Wire::Int(1)));
        let mut b=vec![0xca]; b.extend_from_slice(&1.5f32.to_bits().to_be_bytes());
        assert_eq!(decode_msgpack(&b),Ok(Wire::Float(1.5)));
    }
    #[test] fn msgpack_malformed_and_unsupported() {
        for b in [
            vec![],vec![0xc1],vec![0xa1,0xff],vec![0x82,0xa1,b'a',1,0xa1,b'a',2],
            vec![0x81,1,0xc0],vec![0xd4,1,0],vec![0xc7,0,1],vec![0xc0,0],
            vec![0xdb,0xff,0xff,0xff,0xff],vec![0xcf,1],vec![0x91],vec![0x81,0xa1,b'a'],
            vec![0xcb,0x7f,0xf8,0,0,0,0,0,0],
        ] { assert_eq!(decode_msgpack(&b),Err(RefError),"{b:?}"); }
    }
    #[test] fn msgpack_encode_limits_and_depth() {
        for v in [
            Wire::Int(u64::MAX as i128+1),Wire::Int(i64::MIN as i128-1),
            Wire::Float(f64::NAN),Wire::Float(f64::INFINITY),
            Wire::Text("x".repeat(MAX_BYTES)),Wire::Binary(vec![0;MAX_BYTES]),
            Wire::Array(vec![Wire::Null;MAX_ITEMS+1]),
        ] { assert_eq!(encode_msgpack(&v),Err(RefError)); }
        let mut v=Wire::Null;
        for _ in 0..32 { v=Wire::Array(vec![v]); }
        assert!(encode_msgpack(&v).is_ok());
        assert!(encode_msgpack(&Wire::Array(vec![v])).is_err());
        let mut raw=vec![0x91;33]; raw.push(0xc0);
        assert!(decode_msgpack(&raw).is_err());
        let mut raw=vec![0xdc,0x10,0x01]; raw.extend(vec![0xc0;4097]);
        assert!(decode_msgpack(&raw).is_err());
    }
    #[test] fn external_names_shape_and_collision() {
        assert!(validate_external_names(&BTreeMap::from([("content_type".into(),"content-type".into()),("x".into(),"".into())])).is_ok());
        for v in [
            BTreeMap::from([("a".into(),"same".into()),("b".into(),"same".into())]),
            BTreeMap::from([("bad-name".into(),"ok".into())]),
            BTreeMap::from([("1x".into(),"ok".into())]),
            BTreeMap::from([("a".into(),"x".repeat(MAX_BYTES))]),
        ] { assert!(validate_external_names(&v).is_err()); }
    }
    #[test] fn frame_exact_and_schema_match() {
        let b=encode_atto_binary(&map(1),&SCHEMA).unwrap();
        assert_eq!(&b[..8],b"ATB1\x01\x00\x00\x00");
        assert_eq!(&b[8..40],&SCHEMA);
        assert_eq!(&b[40..44],&[0,0,0,4]);
        assert_eq!(&b[44..],&[0x81,0xa1,b'a',1]);
        assert_eq!(decode_atto_binary(&b,&SCHEMA),Ok(map(1)));
        assert!(decode_atto_binary(&b,&[4;32]).is_err());
        assert!(encode_atto_binary(&map(1),&[0;32]).is_err());
    }
    #[test] fn frame_rejects_length_version_and_noncanonical_payload() {
        let b=encode_atto_binary(&map(1),&SCHEMA).unwrap();
        let mut v=b.clone(); v[4]=2;
        assert!(decode_atto_binary(&v,&SCHEMA).is_err());
        v=b.clone(); v[5]=1;
        assert!(decode_atto_binary(&v,&SCHEMA).is_err());
        v=b.clone(); v.push(0);
        assert!(decode_atto_binary(&v,&SCHEMA).is_err());
        assert!(decode_atto_binary(&b[..b.len()-1],&SCHEMA).is_err());
        // Use a correct frame length so rejection exercises canonicality,
        // not merely the length guard.
        let raw=[0x81,0xa1,b'a',0xcc,1];
        assert_eq!(decode_msgpack(&raw),Ok(map(1)));
        v=b[..40].to_vec(); v.extend_from_slice(&(raw.len() as u32).to_be_bytes()); v.extend_from_slice(&raw);
        assert!(decode_atto_binary(&v,&SCHEMA).is_err());
    }
}
