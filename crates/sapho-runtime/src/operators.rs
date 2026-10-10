// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure logic and identity-preserving collection operations (FR-013..024).
use sapho_core::{Datum, Degree, ErrorCode, Inputs, ItemId, Result, SaphoError, Value};
use sapho_graph::{Comparator, Operation, Reducer};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn operand<'a>(inputs: &'a Inputs, name: &str) -> Result<&'a Value> {
    inputs.get(name).map(|d| &d.value).ok_or_else(|| {
        SaphoError::new(ErrorCode::MissingInput, "Operand absent").with_context("port", name)
    })
}
pub(crate) fn items(value: &Value) -> Result<&[Datum]> {
    if let Value::List(v) = value {
        Ok(v)
    } else {
        Err(mismatch("Operand is not a collection"))
    }
}
/// Select one present Optional without treating false, zero or empty values as absence.
pub(crate) fn merge_present(input: &Inputs) -> Result<Value> {
    let present = input
        .iter()
        .filter_map(|(name, datum)| match &datum.value {
            Value::Optional(Some(value)) => Some(Ok((name.as_str(), value.as_ref()))),
            Value::Optional(None) => None,
            _ => Some(Err(mismatch("MergePresent needs Optional operands"))),
        })
        .collect::<Result<Vec<_>>>()?;
    if present.len() != 1 {
        return Err(SaphoError::new(
            ErrorCode::InvalidValue,
            "MergePresent needs exactly one present operand",
        )
        .with_context("present_count", present.len().to_string())
        .with_context(
            "present_operands",
            present
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    Ok(present[0].1.clone())
}
fn boolean(value: &Value) -> Result<bool> {
    if let Value::Boolean(v) = value {
        Ok(*v)
    } else {
        Err(mismatch("Operand is not Boolean"))
    }
}
fn number(value: &Value) -> Result<f64> {
    match value {
        Value::Number(v) => Ok(*v),
        Value::Probability(v) => Ok(v.get()),
        Value::Degree(v) => Ok(v.get()),
        _ => Err(mismatch("Operand is not a numeric scalar")),
    }
}
fn degree(value: &Value) -> Result<f64> {
    if let Value::Degree(v) = value {
        Ok(v.get())
    } else {
        Err(mismatch("Operand is not Degree"))
    }
}
pub(crate) fn logic(op: &Operation, input: &Inputs) -> Result<Value> {
    Ok(match op {
        Operation::And => {
            Value::Boolean(boolean(operand(input, "a")?)? && boolean(operand(input, "b")?)?)
        }
        Operation::Or => {
            Value::Boolean(boolean(operand(input, "a")?)? || boolean(operand(input, "b")?)?)
        }
        Operation::Not => Value::Boolean(!boolean(operand(input, "value")?)?),
        Operation::Compare { comparator } => {
            let a = operand(input, "a")?;
            let b = operand(input, "b")?;
            Value::Boolean(match comparator {
                Comparator::Equal => a == b,
                Comparator::Less => number(a)? < number(b)?,
                Comparator::LessEqual => number(a)? <= number(b)?,
                Comparator::Greater => number(a)? > number(b)?,
                Comparator::GreaterEqual => number(a)? >= number(b)?,
            })
        }
        Operation::Probability { question, labels } => {
            let Value::Answers(a) = operand(input, "answers")? else {
                return Err(mismatch("Probability needs Answers"));
            };
            Value::Probability(a.probability(question, labels)?)
        }
        Operation::Degree => Value::Degree(Degree::new(number(operand(input, "value")?)?)?),
        Operation::Complement => {
            Value::Degree(Degree::new(1.0 - degree(operand(input, "value")?)?)?)
        }
        Operation::Coalesce => {
            let Value::Optional(v) = operand(input, "value")? else {
                return Err(mismatch("Coalesce needs Optional"));
            };
            match v {
                Some(value) => (**value).clone(),
                None => operand(input, "default")?.clone(),
            }
        }
        Operation::Reduce { reducer, empty } => {
            let values = items(operand(input, "values")?)?;
            if *reducer == Reducer::WeightedMean {
                let weights = items(operand(input, "weights")?)?;
                if weights.len() != values.len()
                    || weights.iter().any(|w| !values.iter().any(|v| v.id == w.id))
                {
                    return Err(mismatch("Weight membership differs from values"));
                }
            }
            if values.is_empty() {
                Value::Degree(*empty)
            } else {
                let ds = values
                    .iter()
                    .map(|d| degree(&d.value))
                    .collect::<Result<Vec<_>>>()?;
                let value = match reducer {
                    Reducer::Min => ds
                        .iter()
                        .copied()
                        .reduce(f64::min)
                        .ok_or_else(|| mismatch("Empty degree reduction"))?,
                    Reducer::Max => ds
                        .iter()
                        .copied()
                        .reduce(f64::max)
                        .ok_or_else(|| mismatch("Empty degree reduction"))?,
                    Reducer::WeightedMean => {
                        let weights = items(operand(input, "weights")?)?;
                        if weights.len() != values.len() {
                            return Err(mismatch("Weight count differs from value count"));
                        }
                        let wm = weights
                            .iter()
                            .map(|d| Ok((&d.id, number(&d.value)?)))
                            .collect::<Result<BTreeMap<_, _>>>()?;
                        let ordered = values
                            .iter()
                            .map(|d| {
                                wm.get(&d.id)
                                    .copied()
                                    .ok_or_else(|| mismatch("Weight identity missing"))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        if ordered.iter().any(|w| *w < 0.0) {
                            return Err(SaphoError::new(
                                ErrorCode::InvalidValue,
                                "Negative weight",
                            ));
                        }
                        let max = ordered
                            .iter()
                            .copied()
                            .reduce(f64::max)
                            .ok_or_else(|| mismatch("Empty weights"))?;
                        if max == 0.0 {
                            return Err(SaphoError::new(
                                ErrorCode::InvalidValue,
                                "Weights sum to zero",
                            ));
                        }
                        let (numerator, denominator) =
                            ds.iter().zip(ordered).fold((0.0, 0.0), |(n, d), (x, w)| {
                                let w = w / max;
                                (n + x * w, d + w)
                            });
                        numerator / denominator
                    }
                };
                Value::Degree(Degree::new(value.clamp(0.0, 1.0))?)
            }
        }
        Operation::Record {}
        | Operation::List { .. }
        | Operation::Code { .. }
        | Operation::Questions { .. }
        | Operation::Ask { .. }
        | Operation::Map { .. }
        | Operation::Filter
        | Operation::Pairs
        | Operation::Join { .. }
        | Operation::Collect
        | Operation::MergePresent {} => return Err(mismatch("Not a logic operation")),
    })
}
pub(crate) fn filtered(inputs: &Inputs) -> Result<Vec<Datum>> {
    let values = items(operand(inputs, "items")?)?;
    let masks = items(operand(inputs, "mask")?)?;
    if values.len() != masks.len() {
        return Err(mismatch("Mask membership differs"));
    }
    let mask = masks
        .iter()
        .map(|d| Ok((&d.id, boolean(&d.value)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    values
        .iter()
        .filter_map(|d| match mask.get(&d.id) {
            Some(true) => Some(Ok(d.clone())),
            Some(false) => None,
            None => Some(Err(mismatch("Mask identity absent"))),
        })
        .collect()
}
pub(crate) fn collected(inputs: &Inputs) -> Result<Vec<Datum>> {
    let outer = items(operand(inputs, "items")?)?;
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for d in outer {
        for inner in items(&d.value)? {
            if !seen.insert(inner.id.clone()) {
                return Err(SaphoError::new(
                    ErrorCode::DuplicateId,
                    "Collected item IDs collide",
                ));
            }
            out.push(inner.clone());
        }
    }
    Ok(out)
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Text(String),
    Boolean(bool),
    Number(u64),
}
fn key(d: &Datum, name: &str) -> Result<Key> {
    let Value::Record(fields) = &d.value else {
        return Err(mismatch("Join needs record items"));
    };
    match fields.get(name) {
        Some(Value::Text(v)) => Ok(Key::Text(v.clone())),
        Some(Value::Boolean(v)) => Ok(Key::Boolean(*v)),
        Some(Value::Number(v)) => Ok(Key::Number(if *v == 0.0 { 0 } else { v.to_bits() })),
        _ => Err(mismatch("Join field absent or not a scalar fact")),
    }
}
pub(crate) fn pair_indices(
    op: &Operation,
    inputs: &Inputs,
    ceiling: usize,
) -> Result<Vec<(usize, usize)>> {
    let left = items(operand(inputs, "left")?)?;
    let right = items(operand(inputs, "right")?)?;
    let mut out = Vec::new();
    match op {
        Operation::Pairs => {
            let count = left
                .len()
                .checked_mul(right.len())
                .filter(|n| *n <= ceiling)
                .ok_or_else(|| {
                    SaphoError::new(ErrorCode::LimitExceeded, "Pair expansion exceeds limit")
                })?;
            out.reserve(count);
            for l in 0..left.len() {
                for rr in 0..right.len() {
                    out.push((l, rr));
                }
            }
        }
        Operation::Join {
            left_key,
            right_key,
        } => {
            let mut index = BTreeMap::<Key, Vec<usize>>::new();
            for (i, d) in right.iter().enumerate() {
                index.entry(key(d, right_key)?).or_default().push(i);
            }
            let keys = left
                .iter()
                .map(|d| key(d, left_key))
                .collect::<Result<Vec<_>>>()?;
            let count = keys.iter().try_fold(0usize, |n, k| {
                n.checked_add(index.get(k).map_or(0, Vec::len))
                    .filter(|n| *n <= ceiling)
                    .ok_or_else(|| {
                        SaphoError::new(ErrorCode::LimitExceeded, "Join expansion exceeds limit")
                    })
            })?;
            out.reserve(count);
            for (l, k) in keys.iter().enumerate() {
                if let Some(rs) = index.get(k) {
                    out.extend(rs.iter().map(|rr| (l, *rr)));
                }
            }
        }
        _ => return Err(mismatch("Not a pair operation")),
    }
    Ok(out)
}
pub(crate) fn pairs(inputs: &Inputs, indices: &[(usize, usize)]) -> Result<Vec<Datum>> {
    let left = items(operand(inputs, "left")?)?;
    let right = items(operand(inputs, "right")?)?;
    indices
        .iter()
        .map(|(l, rr)| {
            let a = left
                .get(*l)
                .ok_or_else(|| mismatch("Left pair index absent"))?;
            let b = right
                .get(*rr)
                .ok_or_else(|| mismatch("Right pair index absent"))?;
            let id = serde_json::to_string(&(a.id.as_str(), b.id.as_str()))
                .map_err(|e| SaphoError::new(ErrorCode::InvalidValue, e.to_string()))?;
            let mut d = Datum {
                id: ItemId::new(id)?,
                value: Value::Record(BTreeMap::from([
                    ("left".into(), a.value.clone()),
                    ("right".into(), b.value.clone()),
                ])),
                sources: Vec::new(),
            };
            d.inherit_sources(a.sources.iter().chain(&b.sources).cloned());
            Ok(d)
        })
        .collect()
}
fn mismatch(message: &str) -> SaphoError {
    SaphoError::new(ErrorCode::TypeMismatch, message)
}
