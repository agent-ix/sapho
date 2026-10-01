// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Strict JSON input and schema-directed conversion; no filesystem or inference.
use crate::{
    Datum, Degree, ErrorCode, Probability, Result, SaphoError, SourceRef, Value, ValueType,
};
use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value as Json};
use std::{cell::Cell, fmt};

/// Decode JSON without duplicate-key collapse, with byte and 128-level structural ceilings.
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
    let value = JsonSeed {
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
    serde_json::from_value(value).map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))
}
struct JsonSeed<'a> {
    depth: usize,
    exhausted: &'a Cell<bool>,
}
impl<'de> DeserializeSeed<'de> for JsonSeed<'_> {
    type Value = Json;
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        decoder: D,
    ) -> std::result::Result<Json, D::Error> {
        if self.depth >= 128 {
            self.exhausted.set(true);
            return Err(serde::de::Error::custom(
                "JSON structural depth exceeds 128",
            ));
        }
        decoder.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for JsonSeed<'_> {
    type Value = Json;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON data without duplicate keys")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Json, E> {
        Ok(Json::Bool(v))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Json, E> {
        Ok(Json::Number(v.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Json, E> {
        Ok(Json::Number(v.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Json, E> {
        Number::from_f64(v)
            .map(Json::Number)
            .ok_or_else(|| E::custom("Non-finite JSON number"))
    }
    fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Json, E> {
        Ok(Json::String(v.into()))
    }
    fn visit_string<E: serde::de::Error>(self, v: String) -> std::result::Result<Json, E> {
        Ok(Json::String(v))
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(JsonSeed {
            depth: self.depth + 1,
            exhausted: self.exhausted,
        })? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Json, A::Error> {
        let mut fields = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(serde::de::Error::custom(format!(
                    "Duplicate JSON key {key:?}"
                )));
            }
            let value = map.next_value_seed(JsonSeed {
                depth: self.depth + 1,
                exhausted: self.exhausted,
            })?;
            fields.insert(key, value);
        }
        Ok(Json::Object(fields))
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
