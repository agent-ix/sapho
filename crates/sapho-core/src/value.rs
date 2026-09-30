// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Values, identity, source attribution and bounded serialization (FR-001/002).
use crate::{Answers, ErrorCode, NamedQuestion, Result, SaphoError, validate_questions};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            /// Create a non-empty identity.
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty() {
                    return Err(SaphoError::new(ErrorCode::InvalidValue, "Empty identity"));
                }
                Ok(Self(value))
            }
            /// Borrow the opaque identity text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
            /// Validate an identity loaded through deserialization.
            pub fn validate(&self) -> Result<()> {
                if self.0.is_empty() {
                    Err(SaphoError::new(ErrorCode::InvalidValue, "Empty identity"))
                } else {
                    Ok(())
                }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
identity!(NodeId, "A graph node identity, distinct from a data item.");
identity!(ItemId, "Stable identity of one collection item.");
identity!(BackendId, "Name of a registered inference binding.");
identity!(PrimitiveId, "Name of a registered native implementation.");
identity!(
    SourceId,
    "Opaque caller-owned source identity; never opened by the engine."
);

macro_rules! unit_scalar {
    ($name:ident, $doc:literal) => {
        #[doc=$doc]
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        #[serde(try_from = "f64", into = "f64")]
        pub struct $name(f64);
        impl $name {
            /// Validate a finite scalar in the unit interval.
            pub fn new(value: f64) -> Result<Self> {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(SaphoError::new(
                        ErrorCode::InvalidValue,
                        "Scalar must be finite in [0,1]",
                    ));
                }
                Ok(Self(value))
            }
            /// Read the scalar without changing its semantic type.
            pub const fn get(self) -> f64 {
                self.0
            }
        }
        impl TryFrom<f64> for $name {
            type Error = SaphoError;
            fn try_from(v: f64) -> Result<Self> {
                Self::new(v)
            }
        }
        impl From<$name> for f64 {
            fn from(v: $name) -> Self {
                v.0
            }
        }
    };
}
unit_scalar!(
    Probability,
    "A model outcome probability; never an aggregate heuristic score."
);
unit_scalar!(
    Degree,
    "A degree used by an explicitly configured heuristic operator."
);

/// Source attribution supplied by the host, using half-open byte offsets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// Opaque source location or reference.
    pub source: SourceId,
    /// Beginning of a range, paired with end, or absent for a whole source.
    pub start: Option<u64>,
    /// Exclusive end of a range.
    pub end: Option<u64>,
}
impl SourceRef {
    /// Check range and identity without accessing a source file.
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        match (self.start, self.end) {
            (None, None) => Ok(()),
            (Some(a), Some(b)) if a <= b => Ok(()),
            _ => Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Invalid source range",
            )),
        }
    }
}

/// Closed port schema; records declare every field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ValueType {
    /// Crisp Boolean.
    Boolean,
    /// Finite numeric fact.
    Number,
    /// Text fact.
    Text,
    /// Model outcome probability.
    Probability,
    /// Heuristic degree.
    Degree,
    /// Explicit absence or a value of a declared type.
    Optional {
        /// Inner present-value type.
        inner: Box<ValueType>,
    },
    /// Identified ordered items.
    List {
        /// Item value type.
        item: Box<ValueType>,
    },
    /// Named fields with exact membership.
    Record {
        /// Field schemas.
        fields: BTreeMap<String, ValueType>,
    },
    /// Ordered named question definitions.
    Questions,
    /// Answers retaining their question spaces.
    Answers,
}
impl ValueType {
    /// Construct a list port.
    pub fn list(item: Self) -> Self {
        Self::List {
            item: Box::new(item),
        }
    }
    /// Construct an optional port.
    pub fn optional(inner: Self) -> Self {
        Self::Optional {
            inner: Box::new(inner),
        }
    }
    /// Check schema depth and field names before recursive evaluation.
    pub fn validate(&self) -> Result<()> {
        self.validate_at(0)
    }
    fn validate_at(&self, depth: usize) -> Result<()> {
        if depth >= 32 {
            return Err(SaphoError::new(
                ErrorCode::LimitExceeded,
                "Type nesting exceeds 32",
            ));
        }
        match self {
            Self::Optional { inner } | Self::List { item: inner } => inner.validate_at(depth + 1),
            Self::Record { fields } => {
                for (name, ty) in fields {
                    validate_name(name)?;
                    ty.validate_at(depth + 1)?;
                }
                Ok(())
            }
            Self::Boolean
            | Self::Number
            | Self::Text
            | Self::Probability
            | Self::Degree
            | Self::Questions
            | Self::Answers => Ok(()),
        }
    }
    /// Check a value against this exact schema.
    pub fn check(&self, value: &Value) -> Result<()> {
        self.validate()?;
        value.validate()?;
        self.check_at(value)
    }
    fn check_at(&self, value: &Value) -> Result<()> {
        match (self, value) {
            (Self::Boolean, Value::Boolean(_))
            | (Self::Number, Value::Number(_))
            | (Self::Text, Value::Text(_))
            | (Self::Probability, Value::Probability(_))
            | (Self::Degree, Value::Degree(_))
            | (Self::Questions, Value::Questions(_))
            | (Self::Answers, Value::Answers(_)) => Ok(()),
            (Self::Optional { .. }, Value::Optional(None)) => Ok(()),
            (Self::Optional { inner }, Value::Optional(Some(v))) => inner.check_at(v),
            (Self::List { item }, Value::List(items)) => {
                for v in items {
                    item.check_at(&v.value)?;
                }
                Ok(())
            }
            (Self::Record { fields }, Value::Record(values)) if fields.keys().eq(values.keys()) => {
                for (key, ty) in fields {
                    let v = values.get(key).ok_or_else(|| {
                        SaphoError::new(ErrorCode::MissingInput, "Record field absent")
                    })?;
                    ty.check_at(v)?;
                }
                Ok(())
            }
            _ => Err(SaphoError::new(
                ErrorCode::TypeMismatch,
                "Value differs from declared port schema",
            )),
        }
    }
}
/// Validate a node, port or record field name.
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(
            SaphoError::new(ErrorCode::InvalidValue, "Invalid node or field name")
                .with_context("name", name),
        )
    } else {
        Ok(())
    }
}

/// A typed value; collection items retain their own identity and sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Value {
    /// Crisp Boolean.
    Boolean(bool),
    /// Finite numeric fact, checked at each boundary.
    Number(f64),
    /// Text fact.
    Text(String),
    /// Model outcome probability.
    Probability(Probability),
    /// Heuristic degree.
    Degree(Degree),
    /// Explicit absence or present value.
    Optional(Option<Box<Value>>),
    /// Ordered identified items.
    List(Vec<Datum>),
    /// Named structured facts.
    Record(BTreeMap<String, Value>),
    /// Ordered model question block.
    Questions(Vec<NamedQuestion>),
    /// Validated answer block with its declared spaces.
    Answers(Answers),
}
impl Value {
    /// Validate nested values independently of a port declaration.
    pub fn validate(&self) -> Result<()> {
        self.validate_at(0)
    }
    fn validate_at(&self, depth: usize) -> Result<()> {
        if depth >= 32 {
            return Err(SaphoError::new(
                ErrorCode::LimitExceeded,
                "Value nesting exceeds 32",
            ));
        }
        match self {
            Self::Number(n) if !n.is_finite() => Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Non-finite number",
            )),
            Self::Optional(Some(v)) => v.validate_at(depth + 1),
            Self::List(items) => {
                let mut seen = BTreeSet::new();
                for d in items {
                    d.id.validate()?;
                    if !seen.insert(&d.id) {
                        return Err(SaphoError::new(
                            ErrorCode::DuplicateId,
                            "Repeated collection item ID",
                        ));
                    }
                    for s in &d.sources {
                        s.validate()?;
                    }
                    d.value.validate_at(depth + 1)?;
                }
                Ok(())
            }
            Self::Record(fields) => {
                for (name, v) in fields {
                    validate_name(name)?;
                    v.validate_at(depth + 1)?;
                }
                Ok(())
            }
            Self::Questions(q) => validate_questions(q),
            Self::Answers(a) => a.validate(),
            Self::Boolean(_)
            | Self::Number(_)
            | Self::Text(_)
            | Self::Probability(_)
            | Self::Degree(_)
            | Self::Optional(None) => Ok(()),
        }
    }
    /// Serialize model state as ordinary JSON, omitting source sidecars and item IDs.
    pub fn to_plain_json(&self) -> Result<serde_json::Value> {
        self.validate()?;
        match self {
            Self::Boolean(v) => Ok((*v).into()),
            Self::Number(v) => Ok(serde_json::json!(v)),
            Self::Text(v) => Ok(v.clone().into()),
            Self::Probability(v) => Ok(serde_json::json!(v.get())),
            Self::Degree(v) => Ok(serde_json::json!(v.get())),
            Self::Optional(None) => Ok(serde_json::Value::Null),
            Self::Optional(Some(v)) => v.to_plain_json(),
            Self::List(items) => items
                .iter()
                .map(|d| d.value.to_plain_json())
                .collect::<Result<Vec<_>>>()
                .map(serde_json::Value::Array),
            Self::Record(fields) => fields
                .iter()
                .map(|(k, v)| Ok((k.clone(), v.to_plain_json()?)))
                .collect::<Result<serde_json::Map<_, _>>>()
                .map(serde_json::Value::Object),
            Self::Questions(v) => serde_json::to_value(v).map_err(serialization_error),
            Self::Answers(v) => serde_json::to_value(v).map_err(serialization_error),
        }
    }
}
/// A value plus caller-visible identity and evidence attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Datum {
    /// Stable collection identity.
    pub id: ItemId,
    /// Typed fact or judgment.
    pub value: Value,
    /// Sources carried separately from model state.
    #[serde(default)]
    pub sources: Vec<SourceRef>,
}
impl Datum {
    /// Construct a checked datum with no initial source references.
    pub fn new(id: impl Into<String>, value: Value) -> Result<Self> {
        let d = Self {
            id: ItemId::new(id)?,
            value,
            sources: Vec::new(),
        };
        d.validate()?;
        Ok(d)
    }
    /// Check the value, identity and attribution.
    pub fn validate(&self) -> Result<()> {
        self.id.validate()?;
        for s in &self.sources {
            s.validate()?;
        }
        self.value.validate()
    }
    /// Merge inherited references without deleting explicitly supplied sources.
    pub fn inherit_sources(&mut self, sources: impl IntoIterator<Item = SourceRef>) {
        let all = self
            .sources
            .iter()
            .cloned()
            .chain(sources)
            .collect::<BTreeSet<_>>();
        self.sources = all.into_iter().collect();
    }
}
/// Named values passed across code and executor ports.
pub type Inputs = BTreeMap<String, Datum>;
/// A signature contains exact named input and output schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// Required named inputs.
    pub inputs: BTreeMap<String, ValueType>,
    /// Required named outputs.
    pub outputs: BTreeMap<String, ValueType>,
}
/// Validate names, datum invariants and exact port membership.
pub fn check_ports(types: &BTreeMap<String, ValueType>, values: &Inputs) -> Result<()> {
    if !types.keys().eq(values.keys()) {
        return Err(SaphoError::new(
            ErrorCode::MissingInput,
            "Port names differ from declared signature",
        ));
    }
    for (name, ty) in types {
        let d = values
            .get(name)
            .ok_or_else(|| SaphoError::new(ErrorCode::MissingInput, "Port absent"))?;
        d.validate()?;
        ty.check(&d.value)?;
    }
    Ok(())
}

struct LimitedWriter {
    bytes: Vec<u8>,
    count: usize,
    max: usize,
    retain: bool,
    exceeded: bool,
}
impl Write for LimitedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self.count.checked_add(buf.len()).filter(|n| *n <= self.max);
        let Some(next) = next else {
            self.exceeded = true;
            return Err(io::Error::other("Serialization limit exceeded"));
        };
        self.count = next;
        if self.retain {
            self.bytes.extend_from_slice(buf);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn serialize_limited<T: Serialize>(value: &T, max: usize, retain: bool) -> Result<LimitedWriter> {
    let mut writer = LimitedWriter {
        bytes: Vec::new(),
        count: 0,
        max,
        retain,
        exceeded: false,
    };
    if let Err(e) = serde_json::to_writer(&mut writer, value) {
        return Err(if writer.exceeded {
            SaphoError::new(ErrorCode::LimitExceeded, "Serialized byte ceiling exceeded")
        } else {
            serialization_error(e)
        });
    }
    Ok(writer)
}
/// Produce JSON bytes without ever growing the buffer beyond the explicit ceiling.
pub fn bounded_json<T: Serialize>(value: &T, max: usize) -> Result<Vec<u8>> {
    Ok(serialize_limited(value, max, true)?.bytes)
}
/// Measure JSON bytes with a bounded counter, without retaining a serialized buffer.
pub fn measured_json_bytes<T: Serialize>(value: &T, max: usize) -> Result<usize> {
    Ok(serialize_limited(value, max, false)?.count)
}
fn serialization_error(e: serde_json::Error) -> SaphoError {
    SaphoError::new(ErrorCode::Config, e.to_string())
}
