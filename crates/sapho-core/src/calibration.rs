// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Checked, versioned calibration value and canonical content identities.
use crate::{
    BackendId, CalibratedProbability, ErrorCode, Probability, Result, SaphoError, SourceId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const FORMAT: &str = "sapho-calibration-v1";
const METHOD: &str = "isotonic-pava-linear-v1";

/// One raw input knot and its fitted, nondecreasing output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationKnot {
    /// Raw model probability.
    pub raw: Probability,
    /// Fitted probability.
    pub calibrated: CalibratedProbability,
}

/// A development-only fit with no case inputs, labels or provider secrets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationMap {
    /// Exact format version.
    pub format: String,
    /// Curated Dataset identity.
    pub dataset_id: SourceId,
    /// Digest of selected development case content.
    pub fit_cases_digest: String,
    /// Digest of the exact fitted observations.
    pub fit_observation_digest: String,
    /// The only permitted fitting split.
    pub split: String,
    /// Semantic identity of the raw fitting graph.
    pub fit_graph_semantic_identity: String,
    /// Raw Probability output name.
    pub raw_output: String,
    /// Contributing backend binding.
    pub fit_binding: BackendId,
    /// Reported actual model name; no weights identity is implied.
    pub fit_actual_model: String,
    /// Exact monotone fitting method.
    pub method: String,
    /// Number of scored development cases.
    pub case_count: u32,
    /// Ascending distinct raw values and nondecreasing fitted values.
    pub knots: Vec<CalibrationKnot>,
    /// Canonical identity of all preceding fields.
    pub map_id: String,
}
impl CalibrationMap {
    /// Construct and validate a map from checked fit components.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dataset_id: SourceId,
        fit_cases_digest: String,
        fit_observation_digest: String,
        fit_graph_semantic_identity: String,
        raw_output: String,
        fit_binding: BackendId,
        fit_actual_model: String,
        case_count: u32,
        knots: Vec<CalibrationKnot>,
    ) -> Result<Self> {
        let mut map = Self {
            format: FORMAT.into(),
            dataset_id,
            fit_cases_digest,
            fit_observation_digest,
            split: "development".into(),
            fit_graph_semantic_identity,
            raw_output,
            fit_binding,
            fit_actual_model,
            method: METHOD.into(),
            case_count,
            knots,
            map_id: String::new(),
        };
        map.validate_fields()?;
        map.map_id = format!(
            "calibration-v1:sha256:{}",
            canonical_sha256("sapho.calibration.v1\n", &map.without_id())?
        );
        Ok(map)
    }
    fn without_id(&self) -> CalibrationMapBody<'_> {
        CalibrationMapBody {
            format: &self.format,
            dataset_id: &self.dataset_id,
            fit_cases_digest: &self.fit_cases_digest,
            fit_observation_digest: &self.fit_observation_digest,
            split: &self.split,
            fit_graph_semantic_identity: &self.fit_graph_semantic_identity,
            raw_output: &self.raw_output,
            fit_binding: &self.fit_binding,
            fit_actual_model: &self.fit_actual_model,
            method: &self.method,
            case_count: self.case_count,
            knots: &self.knots,
        }
    }
    fn validate_fields(&self) -> Result<()> {
        let fail = || SaphoError::new(ErrorCode::InvalidValue, "Invalid calibration map");
        self.dataset_id.validate()?;
        self.fit_binding.validate()?;
        if self.format != FORMAT
            || self.split != "development"
            || self.method != METHOD
            || self.raw_output.trim().is_empty()
            || self.case_count < 20
            || self.knots.len() < 2
            || self.knots.len() > 4096
            || self.fit_actual_model.trim().is_empty()
            || self.fit_actual_model.len() > 256
            || self.fit_actual_model.chars().any(char::is_control)
            || !valid_digest(&self.fit_cases_digest, "dataset-development-v1:sha256:")
            || !valid_digest(
                &self.fit_observation_digest,
                "calibration-observations-v1:sha256:",
            )
            || !valid_digest(&self.fit_graph_semantic_identity, "graph-v1:sha256:")
            || self.knots.windows(2).any(|pair| {
                pair[0].raw.get() >= pair[1].raw.get()
                    || pair[0].calibrated.get() > pair[1].calibrated.get()
            })
        {
            return Err(fail());
        }
        Ok(())
    }
    /// Validate all invariants and recompute the canonical map identity.
    pub fn validate(&self) -> Result<()> {
        self.validate_fields()?;
        let expected = format!(
            "calibration-v1:sha256:{}",
            canonical_sha256("sapho.calibration.v1\n", &self.without_id())?
        );
        if self.map_id != expected {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Calibration map identity differs from content",
            ));
        }
        Ok(())
    }
    /// Apply exact-knot, clamped-end and linear interpolation semantics.
    pub fn apply(&self, raw: Probability) -> Result<CalibratedProbability> {
        self.validate()?;
        let p = raw.get();
        if p <= self.knots[0].raw.get() {
            return Ok(self.knots[0].calibrated);
        }
        for pair in self.knots.windows(2) {
            let x0 = pair[0].raw.get();
            let x1 = pair[1].raw.get();
            if p <= x1 {
                if p == x1 {
                    return Ok(pair[1].calibrated);
                }
                let y0 = pair[0].calibrated.get();
                let y1 = pair[1].calibrated.get();
                return CalibratedProbability::new(
                    (y0 + (p - x0) * (y1 - y0) / (x1 - x0)).clamp(0.0, 1.0),
                );
            }
        }
        Ok(self
            .knots
            .last()
            .expect("validated nonempty knots")
            .calibrated)
    }
}
#[derive(Serialize)]
struct CalibrationMapBody<'a> {
    format: &'a str,
    dataset_id: &'a SourceId,
    fit_cases_digest: &'a str,
    fit_observation_digest: &'a str,
    split: &'a str,
    fit_graph_semantic_identity: &'a str,
    raw_output: &'a str,
    fit_binding: &'a BackendId,
    fit_actual_model: &'a str,
    method: &'a str,
    case_count: u32,
    knots: &'a [CalibrationKnot],
}
fn valid_digest(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
/// Canonical SHA-256 of a typed value under named-map, ordered-array and normalized-number rules.
pub fn canonical_sha256<T: Serialize>(prefix: &str, value: &T) -> Result<String> {
    let mut value = serde_json::to_value(value)
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    normalize(&mut value)?;
    let mut bytes = prefix.as_bytes().to_vec();
    serde_json::to_writer(&mut bytes, &value)
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn normalize(value: &mut Json) -> Result<()> {
    match value {
        Json::Array(items) => {
            for item in items {
                normalize(item)?;
            }
        }
        Json::Object(fields) => {
            let sorted = std::mem::take(fields)
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            for (key, mut item) in sorted {
                normalize(&mut item)?;
                fields.insert(key, item);
            }
        }
        Json::Number(number) if number.is_f64() => {
            let n = number.as_f64().ok_or_else(|| {
                SaphoError::new(ErrorCode::InvalidValue, "Invalid canonical number")
            })?;
            if !n.is_finite() {
                return Err(SaphoError::new(
                    ErrorCode::InvalidValue,
                    "Nonfinite canonical number",
                ));
            }
            if n == 0.0 {
                *number = Number::from(0);
            } else if n.fract() == 0.0 && n >= i64::MIN as f64 && n < i64::MAX as f64 {
                *number = Number::from(n as i64);
            } else if n.fract() == 0.0 && n > 0.0 && n < u64::MAX as f64 {
                *number = Number::from(n as u64);
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, ValueType, decode_plain};
    fn map() -> CalibrationMap {
        let digest = "a".repeat(64);
        CalibrationMap::new(
            SourceId::new("curated").unwrap(),
            format!("dataset-development-v1:sha256:{digest}"),
            format!("calibration-observations-v1:sha256:{digest}"),
            format!("graph-v1:sha256:{digest}"),
            "raw_support".into(),
            BackendId::new("fast_a").unwrap(),
            "model-a".into(),
            20,
            vec![
                CalibrationKnot {
                    raw: Probability::new(0.05).unwrap(),
                    calibrated: CalibratedProbability::new(0.2).unwrap(),
                },
                CalibrationKnot {
                    raw: Probability::new(0.95).unwrap(),
                    calibrated: CalibratedProbability::new(0.8).unwrap(),
                },
            ],
        )
        .unwrap()
    }
    /// Trace: FR-081-AC-1, FR-081-AC-2, FR-081-AC-3, FR-081-AC-5, FR-082-AC-1
    #[test]
    fn map_identity_types_and_interpolation_are_checked() {
        let map = map();
        map.validate().unwrap();
        let parsed: CalibrationMap =
            serde_json::from_value(serde_json::to_value(&map).unwrap()).unwrap();
        assert_eq!(map, parsed);
        assert!(
            ValueType::CalibrationMap
                .check(&Value::CalibrationMap(Box::new(map.clone())))
                .is_ok()
        );
        assert!(
            ValueType::Probability
                .check(&Value::CalibratedProbability(
                    CalibratedProbability::new(0.5).unwrap()
                ))
                .is_err()
        );
        let declared = decode_plain(
            "x",
            &ValueType::CalibratedProbability,
            &serde_json::json!(0.5),
            &[],
        )
        .unwrap();
        assert!(matches!(declared.value, Value::CalibratedProbability(_)));
        for (input, expected) in [(0.0, 0.2), (0.05, 0.2), (0.5, 0.5), (0.95, 0.8), (1.0, 0.8)] {
            assert!(
                (map.apply(Probability::new(input).unwrap()).unwrap().get() - expected).abs()
                    < 1e-12
            );
        }
        let mut changed = map.clone();
        changed.fit_actual_model = "model-b".into();
        assert!(changed.validate().is_err());
        let mut duplicate = map.clone();
        duplicate.knots[1].raw = duplicate.knots[0].raw;
        assert!(duplicate.validate().is_err());
        let mut wrong = serde_json::to_value(&map).unwrap();
        wrong["endpoint"] = serde_json::json!("https://private.invalid");
        assert!(serde_json::from_value::<CalibrationMap>(wrong).is_err());
        let modifications: [fn(&mut CalibrationMap); 8] = [
            |m: &mut CalibrationMap| m.format = "unknown".into(),
            |m: &mut CalibrationMap| m.method = "unknown".into(),
            |m: &mut CalibrationMap| m.split = "held_out".into(),
            |m: &mut CalibrationMap| m.case_count = 19,
            |m: &mut CalibrationMap| {
                m.knots[0].calibrated = CalibratedProbability::new(0.9).unwrap()
            },
            |m: &mut CalibrationMap| m.knots.clear(),
            |m: &mut CalibrationMap| m.fit_cases_digest = "bad".into(),
            |m: &mut CalibrationMap| m.fit_actual_model = "x".repeat(257),
        ];
        for modify in modifications {
            let mut invalid = map.clone();
            modify(&mut invalid);
            assert!(invalid.validate().is_err());
        }
        assert!(Probability::new(f64::NAN).is_err());
        assert!(CalibratedProbability::new(1.1).is_err());
        let renamed = CalibrationMap::new(
            map.dataset_id.clone(),
            map.fit_cases_digest.clone(),
            map.fit_observation_digest.clone(),
            map.fit_graph_semantic_identity.clone(),
            map.raw_output.clone(),
            map.fit_binding.clone(),
            "model-b".into(),
            map.case_count,
            map.knots.clone(),
        )
        .unwrap();
        assert_ne!(map.map_id, renamed.map_id);
        assert!(
            !serde_json::to_string(&map)
                .unwrap()
                .contains("weights_digest")
        );
    }
}
