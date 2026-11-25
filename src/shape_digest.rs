//! Bounded canonical descriptor graph for the restricted actor DATA profiles.
//! Host tooling only; not authentication or compiler-generated descriptor proof.
use serde::Deserialize;
use serde_json::Number;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::{Command, Stdio};

const MAX_INPUT: usize = 4 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor { codec: String, version: Number, root: String, nodes: Vec<Node> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Node { id: String, kind: String, fields: Vec<Field> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Field { slot: Number, name: String, target: String }

fn require(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err("invalid schema descriptor".into()) }
}
fn token(s: &str) -> bool {
    (1..=128).contains(&s.len()) && s.bytes().all(|b|
        b.is_ascii_alphanumeric() || matches!(b,b'_'|b'.'|b':'|b'-'))
}
fn uint(n: &Number, max: u32) -> Result<u32> {
    // Preserve the legacy mathematical integer profile (including 1.0).
    let v = n.as_f64().ok_or("invalid numeric field")?;
    require(v.is_finite() && v >= 0.0 && v <= f64::from(max) && v.fract() == 0.0)?;
    Ok(v as u32)
}
fn allowed(codec: &str, kind: &str) -> bool {
    match codec {
        "cbor" => matches!(kind,"bool"|"i64"|"f64"|"text"|"list"|"record"),
        "protobuf" => matches!(kind,"varint"|"fixed32"|"fixed64"|"bytes"|"message"|"packed"),
        _ => false,
    }
}

/// Parse directly into closed structs so duplicate JSON keys cannot disappear
/// in a generic map before validation. No recursive expansion of graph edges.
pub fn canonical_shape(raw: &[u8]) -> Result<Vec<u8>> {
    require(raw.len() <= MAX_INPUT)?;
    let d: Descriptor = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    require(matches!(d.codec.as_str(),"cbor"|"protobuf") && token(&d.root))?;
    let version = uint(&d.version, u32::MAX)?;
    require(version != 0 && (1..=256).contains(&d.nodes.len()))?;
    let mut table = BTreeMap::new();
    let mut total = 0usize;
    for n in d.nodes {
        require(token(&n.id) && allowed(&d.codec,&n.kind) && !table.contains_key(&n.id))?;
        require(n.fields.len() <= 1024)?;
        total += n.fields.len();
        require(total <= 4096)?;
        let mut names = BTreeSet::new();
        let mut slots = BTreeSet::new();
        let mut fields = Vec::new();
        for f in n.fields {
            let slot = uint(&f.slot,536870911)?;
            require(token(&f.name) && token(&f.target) && names.insert(f.name.clone()) && slots.insert(slot))?;
            if n.kind == "message" {
                require(slot != 0 && !(19000..=19999).contains(&slot))?;
            }
            fields.push((slot,f.name,f.target));
        }
        fields.sort_by_key(|f|f.0);
        match n.kind.as_str() {
            "record" => require(fields.iter().enumerate().all(|(i,f)|f.0 as usize == i))?,
            "list"|"packed" => require(fields.len()==1 && fields[0].0==0 && fields[0].1=="element")?,
            "message" => (),
            _ => require(fields.is_empty())?,
        }
        table.insert(n.id,(n.kind,fields));
    }
    require(table.contains_key(&d.root))?;
    for (kind,fields) in table.values() {
        for (_,_,target) in fields {
            let (target_kind,_) = table.get(target).ok_or("dangling descriptor reference")?;
            if kind == "packed" { require(matches!(target_kind.as_str(),"varint"|"fixed32"|"fixed64"))?; }
        }
    }
    let mut seen = BTreeSet::new();
    let mut work = vec![d.root.as_str()];
    while let Some(id) = work.pop() {
        if !seen.insert(id) { continue; }
        for (_,_,target) in &table.get(id).ok_or("dangling descriptor reference")?.1 {
            work.push(target.as_str());
        }
    }
    require(seen.len()==table.len())?;
    let mut out = format!("ATTO-SHAPE-1\n{}\n{}\n{}\n{}\n",d.codec,version,d.root,table.len());
    for (id,(kind,fields)) in table {
        out.push_str(&format!("{}\n{}\n{}\n",id,kind,fields.len()));
        for (slot,name,target) in fields {
            out.push_str(&format!("{}\n{}\n{}\n",slot,name,target));
        }
    }
    Ok(out.into_bytes())
}

/// Linux host reference: use the reviewed system SHA-256 utility rather
/// than introducing a hand-written cryptographic primitive or lockfile churn.
/// No shell, file name, or untrusted command argument is involved.
pub fn shape_digest(raw: &[u8]) -> Result<String> {
    let canonical = canonical_shape(raw)?;
    let mut child = Command::new("/usr/bin/sha256sum")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e|e.to_string())?;
    let write = child.stdin.take().ok_or("missing digest stdin")
        .and_then(|mut stdin| stdin.write_all(&canonical).map_err(|_|"digest input write failed"));
    if let Err(error) = write {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    let result = child.wait_with_output().map_err(|e|e.to_string())?;
    require(result.status.success())?;
    let output = std::str::from_utf8(&result.stdout).map_err(|e|e.to_string())?;
    let hex = output.strip_suffix("  -\n").ok_or("unexpected digest output")?;
    require(hex.len()==64 && hex.bytes().all(|b|b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))?;
    Ok(hex.into())
}
