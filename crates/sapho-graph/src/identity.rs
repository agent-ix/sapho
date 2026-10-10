// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Versioned identity of parsed graph meaning, separate from source bytes (FR-063).
use crate::{Binding, GraphBody, GraphSpec};
use sapho_core::{ErrorCode, Result, SaphoError};
use serde_json::{Number, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Hash a typed graph with ordered sequences and canonical named maps/numbers.
pub fn graph_semantic_identity(spec: &GraphSpec) -> Result<String> {
    fn checked_body(body: &GraphBody) -> Result<()> {
        for node in &body.nodes {
            for binding in node.inputs.values().chain(node.guard.iter()) {
                checked_binding(binding)?;
            }
        }
        for binding in body.outputs.values() {
            checked_binding(binding)?;
        }
        Ok(())
    }
    fn checked_binding(binding: &Binding) -> Result<()> {
        if let Binding::Literal { value, value_type } = binding {
            value_type.check(&value.value)?;
            value.validate()?;
        }
        Ok(())
    }
    checked_body(&GraphBody {
        inputs: spec.inputs.clone(),
        nodes: spec.nodes.clone(),
        outputs: spec.outputs.clone(),
    })?;
    for body in spec.subgraphs.values() {
        checked_body(body)?;
    }
    let mut value = serde_json::to_value(spec)
        .map_err(|error| SaphoError::new(ErrorCode::Config, error.to_string()))?;
    normalize(&mut value)?;
    let mut bytes = b"sapho.graph.v1\n".to_vec();
    serde_json::to_writer(&mut bytes, &value)
        .map_err(|error| SaphoError::new(ErrorCode::Config, error.to_string()))?;
    Ok(format!("graph-v1:sha256:{:x}", Sha256::digest(bytes)))
}

fn normalize(value: &mut Value) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                normalize(item)?;
            }
        }
        Value::Object(fields) => {
            let sorted = std::mem::take(fields)
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            for (key, mut item) in sorted {
                normalize(&mut item)?;
                fields.insert(key, item);
            }
        }
        Value::Number(number) => {
            if number.is_f64()
                && let Some(value) = number.as_f64()
            {
                if !value.is_finite() {
                    return Err(SaphoError::new(
                        ErrorCode::InvalidValue,
                        "Nonfinite graph number",
                    ));
                }
                if value == 0.0 {
                    *number = Number::from(0);
                } else if value.fract() == 0.0
                    && value >= i64::MIN as f64
                    && value < i64::MAX as f64
                {
                    *number = Number::from(value as i64);
                } else if value.fract() == 0.0 && value > 0.0 && value < u64::MAX as f64 {
                    *number = Number::from(value as u64);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Trace: FR-063-AC-2, IT-009-SC-04
    #[test]
    fn nested_named_maps_sort_keys_independent_of_serde_json_map_feature() {
        let mut left = serde_json::Map::new();
        left.insert("z".into(), serde_json::json!({"b": 2, "a": 1}));
        left.insert("a".into(), serde_json::json!([{"y": 2, "x": 1}]));
        let mut right = serde_json::Map::new();
        right.insert("a".into(), serde_json::json!([{"x": 1, "y": 2}]));
        right.insert("z".into(), serde_json::json!({"a": 1, "b": 2}));
        let mut left = Value::Object(left);
        let mut right = Value::Object(right);
        normalize(&mut left).unwrap();
        normalize(&mut right).unwrap();
        assert_eq!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );
    }
}
