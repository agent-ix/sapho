// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Strict JSON input and schema-directed conversion; no filesystem or inference.
use crate::{
    Datum, Degree, ErrorCode, Probability, Result, SaphoError, SourceRef, Value, ValueType,
};
use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value as Json;
use std::{cell::Cell, collections::BTreeSet, fmt};

/// Decode JSON without duplicate-key collapse, with byte and 128-level structural ceilings.
///
/// The document is first checked for depth and duplicate keys by a pass that keeps no
/// values, then decoded straight into `T`, so no generic JSON tree of the whole document is
/// ever built and a large Dataset costs memory in proportion to its typed form only.
pub fn decode_json<T: DeserializeOwned>(bytes: &[u8], max_bytes: usize) -> Result<T> {
    if bytes.len() > max_bytes {
        return Err(SaphoError::new(
            ErrorCode::LimitExceeded,
            "JSON exceeds byte ceiling",
        ));
    }
    let exhausted = Cell::new(false);
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    // Our seed checks depth before descending, including ignored/unknown fields.
    decoder.disable_recursion_limit();
    Shape {
        depth: 0,
        exhausted: &exhausted,
    }
    .deserialize(&mut decoder)
    .map_err(|e| {
        SaphoError::new(
            if exhausted.get() {
                ErrorCode::LimitExceeded
            } else {
                ErrorCode::Config
            },
            e.to_string(),
        )
    })?;
    decoder
        .end()
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    // The depth is bounded by the check above, so the typed pass needs no limit of its own.
    let mut typed = serde_json::Deserializer::from_slice(bytes);
    typed.disable_recursion_limit();
    let value = T::deserialize(&mut typed)
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    typed
        .end()
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    Ok(value)
}
/// Walks a document, refusing excessive depth and duplicate keys, and keeps nothing.
struct Shape<'a> {
    depth: usize,
    exhausted: &'a Cell<bool>,
}
impl<'de> DeserializeSeed<'de> for Shape<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        decoder: D,
    ) -> std::result::Result<(), D::Error> {
        if self.depth >= 128 {
            self.exhausted.set(true);
            return Err(serde::de::Error::custom(
                "JSON structural depth exceeds 128",
            ));
        }
        decoder.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Shape<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON data without duplicate keys")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    // The JSON reader refuses a number outside the finite range itself.
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        while seq
            .next_element_seed(Shape {
                depth: self.depth + 1,
                exhausted: self.exhausted,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "Duplicate JSON key {key:?}"
                )));
            }
            map.next_value_seed(Shape {
                depth: self.depth + 1,
                exhausted: self.exhausted,
            })?;
        }
        Ok(())
    }
}

/// Convert plain JSON according to a declared schema, retaining caller identity and sources.
pub fn decode_plain(
    id: impl Into<String>,
    schema: &ValueType,
    json: &Json,
    sources: &[SourceRef],
) -> Result<Datum> {
    schema.validate()?;
    for source in sources {
        source.validate()?;
    }
    let id = id.into();
    let value = convert(schema, json, std::slice::from_ref(&id), sources, 0)?;
    schema.check(&value)?;
    let mut datum = Datum::new(id, value)?;
    datum.inherit_sources(sources.iter().cloned());
    Ok(datum)
}
fn convert(
    schema: &ValueType,
    json: &Json,
    path: &[String],
    sources: &[SourceRef],
    depth: usize,
) -> Result<Value> {
    if depth >= 32 {
        return Err(SaphoError::new(
            ErrorCode::LimitExceeded,
            "Plain value nesting exceeds 32",
        ));
    }
    let mismatch = || {
        SaphoError::new(
            ErrorCode::TypeMismatch,
            "Plain data differs from declared schema",
        )
    };
    Ok(match schema {
        ValueType::Boolean => Value::Boolean(json.as_bool().ok_or_else(mismatch)?),
        ValueType::Number => Value::Number(json.as_f64().ok_or_else(mismatch)?),
        ValueType::Text => Value::Text(json.as_str().ok_or_else(mismatch)?.into()),
        ValueType::Probability => {
            Value::Probability(Probability::new(json.as_f64().ok_or_else(mismatch)?)?)
        }
        ValueType::Degree => Value::Degree(Degree::new(json.as_f64().ok_or_else(mismatch)?)?),
        ValueType::Optional { inner } => {
            if json.is_null() {
                Value::Optional(None)
            } else {
                Value::Optional(Some(Box::new(convert(
                    inner,
                    json,
                    path,
                    sources,
                    depth + 1,
                )?)))
            }
        }
        ValueType::Record { fields } => {
            let values = json.as_object().ok_or_else(mismatch)?;
            if fields.len() != values.len() || fields.keys().any(|name| !values.contains_key(name))
            {
                return Err(mismatch());
            }
            Value::Record(
                fields
                    .iter()
                    .map(|(name, ty)| {
                        let json = values.get(name).ok_or_else(mismatch)?;
                        let mut child = path.to_vec();
                        child.push(name.clone());
                        Ok((name.clone(), convert(ty, json, &child, sources, depth + 1)?))
                    })
                    .collect::<Result<_>>()?,
            )
        }
        ValueType::List { item } => {
            let values = json.as_array().ok_or_else(mismatch)?;
            Value::List(
                values
                    .iter()
                    .enumerate()
                    .map(|(index, json)| {
                        let mut child = path.to_vec();
                        child.push(index.to_string());
                        let id = serde_json::to_string(&child)
                            .map_err(|e| SaphoError::new(ErrorCode::InvalidValue, e.to_string()))?;
                        let mut datum =
                            Datum::new(id, convert(item, json, &child, sources, depth + 1)?)?;
                        datum.inherit_sources(sources.iter().cloned());
                        Ok(datum)
                    })
                    .collect::<Result<_>>()?,
            )
        }
        ValueType::Questions => {
            Value::Questions(serde_json::from_value(json.clone()).map_err(|_| mismatch())?)
        }
        ValueType::Answers => {
            Value::Answers(serde_json::from_value(json.clone()).map_err(|_| mismatch())?)
        }
    })
}
