//! Strict Rust reference JSON ingress for draft Atto contracts.
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const MAX_WIRE_BYTES: usize = 1_048_576;

#[derive(Debug)]
struct Unique(Value);

impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("strict JSON without duplicate properties or floats")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Unique, E> {
                Err(E::custom("JSON floating-point syntax is not supported by this contract"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Unique, D::Error> {
                Unique::deserialize(d)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<Unique>()? {
                    values.push(value.0);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                let mut values = Map::new();
                while let Some((key, value)) = map.next_entry::<String, Unique>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom(format!("duplicate property: {key}")));
                    }
                    values.insert(key, value.0);
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

/// A successful parse only means that a JSON value satisfies a draft contract;
/// it does not authorize an action or establish runtime conformance.
pub fn decode_and_validate(raw: &[u8], schema: &Value) -> Result<Value, String> {
    if raw.len() > MAX_WIRE_BYTES {
        return Err("wire payload exceeds one MiB".into());
    }
    let mut decoder = serde_json::Deserializer::from_slice(raw);
    let value = Unique::deserialize(&mut decoder).map_err(|e| e.to_string())?.0;
    decoder.end().map_err(|e| e.to_string())?;
    if !value.is_object() {
        return Err("expected top-level JSON object".into());
    }
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|e| e.to_string())?;
    validator.validate(&value).map_err(|e| e.to_string())?;
    Ok(value)
}

pub fn wit_fields(wit: &str, name: &str) -> BTreeMap<String, String> {
    let marker = format!("record {name} {{");
    let body = wit.split_once(&marker).expect("WIT record missing")
        .1.split_once('}').expect("unterminated WIT record").0;
    body.lines().filter_map(|line| {
        let line = line.trim().trim_end_matches(',');
        line.split_once(':').map(|(name, ty)| (name.trim().into(), ty.trim().into()))
    }).collect()
}

pub fn typespec_fields(tsp: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut result: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in tsp.lines() {
        let line = line.trim();
        if !line.ends_with(';') || line.starts_with('@') || line.starts_with("//") {
            continue;
        }
        if let Some((name, typ)) = line.trim_end_matches(';').split_once(':') {
            let name = name.trim();
            if name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                result.entry(name.into()).or_default().insert(typ.trim().into());
            }
        }
    }
    result
}

pub mod shape_digest;

pub mod serde_receipts;
