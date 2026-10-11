// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure, versioned held-out promotion registration and bar evaluation.
use crate::{Dataset, EvidenceError, Measurement, Metrics, OutputMeasurement, Split};
use sapho_core::{SourceId, Value, ValueType};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Mandatory thresholds for one public graph output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateBars {
    /// Exclusive lower bound on improvement over the best constant predictor.
    pub minimum_margin: f64,
    /// Inclusive upper bound on the shared decile ECE.
    pub maximum_ece: f64,
    /// Inclusive positive true-class recall floor.
    pub minimum_true_recall: f64,
    /// Inclusive positive false-class recall floor.
    pub minimum_false_recall: f64,
    /// Inclusive maximum fraction of cases with differing repeated predictions.
    pub maximum_disagreement: f64,
    /// Inclusive positive scored-case floor.
    pub minimum_scored_count: u32,
}
impl GateBars {
    fn validate(&self) -> Result<(), EvidenceError> {
        let unit = |n: f64| n.is_finite() && (0.0..=1.0).contains(&n);
        if !unit(self.minimum_margin)
            || !unit(self.maximum_ece)
            || !unit(self.minimum_true_recall)
            || self.minimum_true_recall == 0.0
            || !unit(self.minimum_false_recall)
            || self.minimum_false_recall == 0.0
            || !unit(self.maximum_disagreement)
            || self.minimum_scored_count == 0
        {
            return Err(EvidenceError::InvalidGate("Invalid mandatory bar".into()));
        }
        Ok(())
    }
}
/// Exact held-out Dataset, graph and public output contract chosen before evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionGate {
    /// Schema version, currently one.
    pub version: u32,
    /// Caller-owned Dataset identity.
    pub dataset: SourceId,
    /// Canonical digest of all validated Dataset content.
    pub dataset_content_digest: String,
    /// Must be held_out.
    pub split: Split,
    /// Semantic graph identity, independent of source representation.
    pub graph_semantic_identity: String,
    /// Exactly the public output-name set.
    #[serde(deserialize_with = "unique_outputs")]
    pub outputs: BTreeMap<String, GateBars>,
}
fn unique_outputs<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, GateBars>, D::Error> {
    use serde::de::{Error, MapAccess, Visitor};
    struct Unique;
    impl<'de> Visitor<'de> for Unique {
        type Value = BTreeMap<String, GateBars>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("unique named output bars")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut outputs = BTreeMap::new();
            while let Some((name, bars)) = map.next_entry::<String, GateBars>()? {
                if outputs.insert(name, bars).is_some() {
                    return Err(M::Error::custom("duplicate gate output name"));
                }
            }
            Ok(outputs)
        }
    }
    deserializer.deserialize_map(Unique)
}
impl PromotionGate {
    /// Refuse malformed, nonpromotable or incomplete gate declarations.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        self.dataset.validate()?;
        if self.version != 1
            || self.split != Split::HeldOut
            || !self
                .dataset_content_digest
                .starts_with("dataset-v1:sha256:")
            || !self.graph_semantic_identity.starts_with("graph-v1:sha256:")
            || self.outputs.is_empty()
            || self.outputs.len() > 128
        {
            return Err(EvidenceError::InvalidGate(
                "Invalid gate identity, split or output set".into(),
            ));
        }
        for (name, bars) in &self.outputs {
            sapho_core::validate_name(name)?;
            bars.validate()?;
        }
        Ok(())
    }
}
/// Local exclusive receipt of a validated gate snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRegistration {
    /// Complete registered thresholds and bound identities.
    pub gate: PromotionGate,
    /// Lowercase SHA-256 of exact gate file bytes.
    pub gate_bytes_digest: String,
    /// Versioned semantic identity of the typed gate.
    pub gate_semantic_identity: String,
}

fn canonical(mut value: Json) -> Result<Vec<u8>, EvidenceError> {
    fn normalize(value: &mut Json) -> Result<(), EvidenceError> {
        match value {
            Json::Object(fields) => {
                let sorted = std::mem::take(fields)
                    .into_iter()
                    .collect::<BTreeMap<_, _>>();
                for (key, mut item) in sorted {
                    normalize(&mut item)?;
                    fields.insert(key, item);
                }
            }
            Json::Array(items) => {
                for item in items {
                    normalize(item)?;
                }
            }
            Json::Number(number) if number.is_f64() => {
                let n = number
                    .as_f64()
                    .ok_or_else(|| EvidenceError::InvalidGate("Invalid number".into()))?;
                if !n.is_finite() {
                    return Err(EvidenceError::InvalidGate("Nonfinite number".into()));
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
    normalize(&mut value)?;
    serde_json::to_vec(&value).map_err(|e| EvidenceError::InvalidGate(e.to_string()))
}
fn semantic<T: Serialize>(value: &T, domain: &[u8], prefix: &str) -> Result<String, EvidenceError> {
    let json =
        serde_json::to_value(value).map_err(|e| EvidenceError::InvalidGate(e.to_string()))?;
    let mut bytes = domain.to_vec();
    bytes.extend(canonical(json)?);
    Ok(format!("{prefix}{:x}", Sha256::digest(bytes)))
}
/// Canonical content identity of a validated Dataset, preserving case order.
pub fn dataset_content_digest(
    dataset: &Dataset,
    max_cases: usize,
) -> Result<String, EvidenceError> {
    dataset.validate(max_cases)?;
    semantic(dataset, b"sapho.dataset.v1\n", "dataset-v1:sha256:")
}
/// Validate and snapshot exact gate bytes without host I/O.
pub fn register_gate(
    bytes: &[u8],
    dataset: &Dataset,
    graph_identity: &str,
    graph_outputs: &BTreeMap<String, ValueType>,
    max_cases: usize,
) -> Result<GateRegistration, EvidenceError> {
    let gate: PromotionGate =
        serde_json::from_slice(bytes).map_err(|e| EvidenceError::InvalidGate(e.to_string()))?;
    gate.validate()?;
    let names = graph_outputs.keys().collect::<BTreeSet<_>>();
    if gate.outputs.keys().collect::<BTreeSet<_>>() != names
        || gate.dataset != dataset.id
        || gate.dataset_content_digest != dataset_content_digest(dataset, max_cases)?
        || gate.graph_semantic_identity != graph_identity
    {
        return Err(EvidenceError::InvalidGate(
            "Gate Dataset, graph or output set mismatch".into(),
        ));
    }
    Ok(GateRegistration {
        gate_semantic_identity: semantic(&gate, b"sapho.gate.v1\n", "gate-v1:sha256:")?,
        gate_bytes_digest: format!("{:x}", Sha256::digest(bytes)),
        gate,
    })
}
/// Validate an existing receipt against the current file before any evaluation.
pub fn verify_gate(
    registration: &GateRegistration,
    bytes: &[u8],
    dataset: &Dataset,
    graph_identity: &str,
    graph_outputs: &BTreeMap<String, ValueType>,
    max_cases: usize,
) -> Result<(), EvidenceError> {
    let current = register_gate(bytes, dataset, graph_identity, graph_outputs, max_cases)?;
    if &current != registration {
        return Err(EvidenceError::InvalidGate(
            "Registered gate bytes or snapshot changed".into(),
        ));
    }
    Ok(())
}
/// One mandatory observed bar, with its comparison and absence reason.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarResult {
    /// Observed metric, absent when evidence cannot compute it.
    pub observed: Option<f64>,
    /// Pre-registered threshold.
    pub threshold: f64,
    /// Exact comparator.
    pub comparator: &'static str,
    /// Pass, fail or not_computed.
    pub status: BarStatus,
    /// Why the value is absent.
    pub reason: Option<&'static str>,
}
/// Three-way bar status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BarStatus {
    /// Observed value meets the threshold.
    Pass,
    /// Observed value misses the threshold.
    Fail,
    /// Required evidence is absent.
    NotComputed,
}
fn bar(
    observed: Option<f64>,
    threshold: f64,
    comparator: &'static str,
    reason: &'static str,
) -> BarResult {
    let status = match observed {
        None => BarStatus::NotComputed,
        Some(value)
            if match comparator {
                ">" => value > threshold,
                "<=" => value <= threshold,
                ">=" => value >= threshold,
                _ => false,
            } =>
        {
            BarStatus::Pass
        }
        Some(_) => BarStatus::Fail,
    };
    BarResult {
        observed,
        threshold,
        comparator,
        status,
        reason: (status == BarStatus::NotComputed).then_some(reason),
    }
}
/// Every threshold result and its denominators for one public output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputGateReport {
    /// Selected held-out cases.
    pub selected: u32,
    /// Cases with this declared label.
    pub labelled: u32,
    /// Scored primary predictions.
    pub scored: u32,
    /// Best true/false constant predictor agreement.
    pub best_constant_predictor: Option<f64>,
    /// Exclusive margin over the best constant predictor.
    pub margin: BarResult,
    /// Shared Probability ECE.
    pub ece: BarResult,
    /// True-label recall.
    pub true_recall: BarResult,
    /// False-label recall.
    pub false_recall: BarResult,
    /// Fraction of cases with disagreement over all runs.
    pub disagreement: BarResult,
    /// Count of scored primary cases.
    pub scored_count: BarResult,
}
/// Pure result; host retains measurement coverage and assigns process code.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GateReport {
    /// Registered exact gate-file digest.
    pub gate_bytes_digest: String,
    /// Registered typed semantic identity.
    pub gate_semantic_identity: String,
    /// Dataset ID.
    pub dataset: SourceId,
    /// Dataset content identity.
    pub dataset_content_digest: String,
    /// Explicit held-out split.
    pub split: Split,
    /// Graph semantic identity.
    pub graph_semantic_identity: String,
    /// Per-output complete bar rows.
    pub outputs: BTreeMap<String, OutputGateReport>,
    /// Whether every selected case has a label and scored result in all runs.
    pub complete: bool,
    /// Whole-graph result.
    pub verdict: GateVerdict,
}
/// Whole-graph verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    /// Every output and bar passed with complete evidence.
    Go,
    /// At least one bar failed or was unavailable.
    NoGo,
}
fn prediction(value: &Value) -> Option<bool> {
    match value {
        Value::Boolean(value) => Some(*value),
        Value::Probability(value) => Some(value.get() >= 0.5),
        _ => None,
    }
}
/// Evaluate all six bars for every output without effects or early bar exit.
pub fn evaluate_gate(
    registration: &GateRegistration,
    dataset: &Dataset,
    graph_identity: &str,
    graph_outputs: &BTreeMap<String, ValueType>,
    primary: &Measurement,
    repetitions: &[Measurement],
    max_cases: usize,
) -> Result<GateReport, EvidenceError> {
    let gate = &registration.gate;
    gate.validate()?;
    if registration.gate_semantic_identity != semantic(gate, b"sapho.gate.v1\n", "gate-v1:sha256:")?
    {
        return Err(EvidenceError::InvalidGate(
            "Receipt gate snapshot identity mismatch".into(),
        ));
    }
    if gate.dataset != dataset.id
        || gate.dataset_content_digest != dataset_content_digest(dataset, max_cases)?
        || gate.graph_semantic_identity != graph_identity
        || primary.dataset != dataset.id
        || primary.split != Split::HeldOut
        || repetitions
            .iter()
            .any(|r| r.dataset != dataset.id || r.split != Split::HeldOut)
        || gate.outputs.keys().collect::<BTreeSet<_>>()
            != graph_outputs.keys().collect::<BTreeSet<_>>()
    {
        return Err(EvidenceError::InvalidGate(
            "Measurement identity or output set mismatch".into(),
        ));
    }
    let selected = dataset.selected(Split::HeldOut).collect::<Vec<_>>();
    let mut complete = !selected.is_empty() && repetitions.len() >= 2;
    let mut outputs = BTreeMap::new();
    for (name, bars) in &gate.outputs {
        let absent = OutputMeasurement {
            labelled: 0,
            scored: 0,
            unscored: 0,
            failed: 0,
            metrics: Metrics::Unsupported {
                value_type: graph_outputs.get(name).cloned(),
            },
        };
        let measured = primary.outputs.get(name).unwrap_or(&absent);
        let mut scored = 0u32;
        let mut correct = 0u32;
        let mut true_labels = 0u32;
        let mut false_labels = 0u32;
        let mut true_correct = 0u32;
        let mut false_correct = 0u32;
        let mut disagree = 0u32;
        let mut output_complete =
            primary.outputs.contains_key(name) && measured.failed == 0 && measured.unscored == 0;
        for case in &selected {
            let Some(label) = case.labels.get(name) else {
                output_complete = false;
                continue;
            };
            let primary_value = primary
                .predictions
                .iter()
                .find(|p| p.case == case.id && p.output == *name)
                .and_then(|p| p.predicted.as_ref())
                .and_then(prediction);
            let Some(predicted) = primary_value else {
                output_complete = false;
                continue;
            };
            scored += 1;
            if *label {
                true_labels += 1;
                if predicted {
                    true_correct += 1;
                }
            } else {
                false_labels += 1;
                if !predicted {
                    false_correct += 1;
                }
            }
            if predicted == *label {
                correct += 1;
            }
            let mut all_same = true;
            for run in repetitions {
                let repeated = run
                    .predictions
                    .iter()
                    .find(|p| p.case == case.id && p.output == *name)
                    .and_then(|p| p.predicted.as_ref())
                    .and_then(prediction);
                match repeated {
                    Some(value) if value == predicted => {}
                    Some(_) => all_same = false,
                    None => output_complete = false,
                }
            }
            if !all_same {
                disagree += 1;
            }
        }
        if scored != measured.scored
            || usize::try_from(scored).ok() != Some(selected.len())
            || repetitions.iter().any(|r| {
                r.outputs
                    .get(name)
                    .is_none_or(|o| o.failed != 0 || o.unscored != 0 || o.scored != scored)
            })
        {
            output_complete = false;
        }
        complete &= output_complete;
        let baseline =
            (scored > 0).then(|| f64::from(true_labels.max(false_labels)) / f64::from(scored));
        let margin = baseline.map(|base| f64::from(correct) / f64::from(scored) - base);
        let ece = match &measured.metrics {
            Metrics::Probability { ece, .. } => *ece,
            _ => None,
        };
        let true_recall =
            (true_labels > 0).then(|| f64::from(true_correct) / f64::from(true_labels));
        let false_recall =
            (false_labels > 0).then(|| f64::from(false_correct) / f64::from(false_labels));
        let disagreement = (repetitions.len() >= 2 && output_complete)
            .then(|| f64::from(disagree) / f64::from(scored));
        outputs.insert(
            name.clone(),
            OutputGateReport {
                selected: u32::try_from(selected.len()).unwrap_or(u32::MAX),
                labelled: measured.labelled,
                scored,
                best_constant_predictor: baseline,
                margin: bar(margin, bars.minimum_margin, ">", "no_scored_cases"),
                ece: bar(ece, bars.maximum_ece, "<=", "ece_unavailable"),
                true_recall: bar(
                    true_recall,
                    bars.minimum_true_recall,
                    ">=",
                    "no_true_labels",
                ),
                false_recall: bar(
                    false_recall,
                    bars.minimum_false_recall,
                    ">=",
                    "no_false_labels",
                ),
                disagreement: bar(
                    disagreement,
                    bars.maximum_disagreement,
                    "<=",
                    "repetitions_incomplete",
                ),
                scored_count: bar(
                    (scored > 0).then_some(f64::from(scored)),
                    f64::from(bars.minimum_scored_count),
                    ">=",
                    "no_scored_cases",
                ),
            },
        );
    }
    let all_pass = outputs.values().all(|o| {
        [
            &o.margin,
            &o.ece,
            &o.true_recall,
            &o.false_recall,
            &o.disagreement,
            &o.scored_count,
        ]
        .iter()
        .all(|bar| bar.status == BarStatus::Pass)
    });
    Ok(GateReport {
        gate_bytes_digest: registration.gate_bytes_digest.clone(),
        gate_semantic_identity: registration.gate_semantic_identity.clone(),
        dataset: dataset.id.clone(),
        dataset_content_digest: gate.dataset_content_digest.clone(),
        split: Split::HeldOut,
        graph_semantic_identity: graph_identity.into(),
        outputs,
        complete,
        verdict: if complete && all_pass {
            GateVerdict::Go
        } else {
            GateVerdict::NoGo
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Case, CaseOutcome, LabelKind, LabelProvenance, measure};
    use sapho_core::{Datum, Inputs, ItemId, Probability};
    fn fixture(labels: &[bool]) -> Dataset {
        Dataset {
            id: SourceId::new("synthetic").unwrap(),
            cases: labels
                .iter()
                .enumerate()
                .map(|(i, label)| Case {
                    id: ItemId::new(format!("case-{i}")).unwrap(),
                    split: Split::HeldOut,
                    inputs: Inputs::new(),
                    labels: BTreeMap::from([("result".into(), *label)]),
                    label_provenance: LabelProvenance {
                        kind: LabelKind::Human,
                        source: "fixture".into(),
                        reference: "fixture".into(),
                    },
                })
                .collect(),
        }
    }
    fn schemas() -> BTreeMap<String, ValueType> {
        BTreeMap::from([("result".into(), ValueType::Probability)])
    }
    fn bars() -> GateBars {
        GateBars {
            minimum_margin: 0.0,
            maximum_ece: 0.5,
            minimum_true_recall: 0.5,
            minimum_false_recall: 0.5,
            maximum_disagreement: 0.25,
            minimum_scored_count: 2,
        }
    }
    fn gate(data: &Dataset, bars: GateBars) -> Vec<u8> {
        serde_json::to_vec(&PromotionGate {
            version: 1,
            dataset: data.id.clone(),
            dataset_content_digest: dataset_content_digest(data, 100).unwrap(),
            split: Split::HeldOut,
            graph_semantic_identity: "graph-v1:sha256:fixture".into(),
            outputs: BTreeMap::from([("result".into(), bars)]),
        })
        .unwrap()
    }
    fn measured(data: &Dataset, values: &[f64]) -> Measurement {
        let outcomes = values
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    ItemId::new(format!("case-{i}")).unwrap(),
                    CaseOutcome::Completed {
                        outputs: Inputs::from([(
                            "result".into(),
                            Datum::new("p", Value::Probability(Probability::new(*p).unwrap()))
                                .unwrap(),
                        )]),
                        models: vec![],
                    },
                )
            })
            .collect();
        measure(data, Split::HeldOut, &schemas(), &outcomes, 100).unwrap()
    }
    /// Trace: FR-070-AC-3, FR-070-AC-6, IT-012-SC-04
    #[test]
    fn absent_classes_scores_and_repetitions_remain_not_computed() {
        let data = fixture(&[false, false]);
        let bytes = gate(&data, bars());
        let receipt =
            register_gate(&bytes, &data, "graph-v1:sha256:fixture", &schemas(), 100).unwrap();
        let complete = measured(&data, &[0.1, 0.2]);
        let one_repeat = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &complete,
            std::slice::from_ref(&complete),
            100,
        )
        .unwrap();
        assert!(!one_repeat.complete);
        assert_eq!(
            one_repeat.outputs["result"].true_recall.status,
            BarStatus::NotComputed
        );
        assert_eq!(
            one_repeat.outputs["result"].disagreement.status,
            BarStatus::NotComputed
        );
        let no_scores = measured(&data, &[]);
        let absent = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &no_scores,
            &[no_scores.clone(), no_scores.clone()],
            100,
        )
        .unwrap();
        assert_eq!(
            absent.outputs["result"].scored_count.status,
            BarStatus::NotComputed
        );
        assert_eq!(
            absent.outputs["result"].margin.status,
            BarStatus::NotComputed
        );
        assert!(
            evaluate_gate(
                &receipt,
                &data,
                "graph-v1:sha256:other",
                &schemas(),
                &complete,
                &[complete.clone(), complete.clone()],
                100
            )
            .is_err()
        );
    }
    /// Trace: FR-070-AC-1, FR-070-AC-2, IT-012-SC-02
    #[test]
    fn four_case_best_constant_tie_has_zero_margin_and_true_recall() {
        let data = fixture(&[false, false, false, true]);
        let receipt = register_gate(
            &gate(&data, bars()),
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            100,
        )
        .unwrap();
        let primary = measured(&data, &[0.1, 0.1, 0.1, 0.1]);
        let report = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &primary,
            &[primary.clone(), primary.clone()],
            100,
        )
        .unwrap();
        let output = &report.outputs["result"];
        assert!(report.complete);
        assert_eq!(output.selected, 4);
        assert_eq!(output.scored, 4);
        assert_eq!(output.best_constant_predictor, Some(0.75));
        assert_eq!(output.margin.observed, Some(0.0));
        assert_eq!(output.margin.status, BarStatus::Fail);
        assert_eq!(output.true_recall.observed, Some(0.0));
        assert_eq!(output.true_recall.status, BarStatus::Fail);
        assert_eq!(report.verdict, GateVerdict::NoGo);
    }
    /// Trace: FR-069-AC-1, FR-069-AC-2, FR-069-AC-3, FR-069-AC-4, FR-069-AC-5, FR-069-AC-6, IT-012-SC-01, IT-012-SC-05
    #[test]
    fn receipt_binds_exact_bytes_semantics_dataset_and_complete_output_set() {
        let data = fixture(&[true, false]);
        let bytes = gate(&data, bars());
        let registered =
            register_gate(&bytes, &data, "graph-v1:sha256:fixture", &schemas(), 100).unwrap();
        assert_eq!(registered.gate.dataset, data.id);
        let pretty = serde_json::to_vec_pretty(&registered.gate).unwrap();
        let reformatted =
            register_gate(&pretty, &data, "graph-v1:sha256:fixture", &schemas(), 100).unwrap();
        assert_ne!(registered.gate_bytes_digest, reformatted.gate_bytes_digest);
        assert_eq!(
            registered.gate_semantic_identity,
            reformatted.gate_semantic_identity
        );
        assert!(
            verify_gate(
                &registered,
                &pretty,
                &data,
                "graph-v1:sha256:fixture",
                &schemas(),
                100
            )
            .is_err()
        );
        let mut changed = data.clone();
        changed.cases[0].labels.insert("result".into(), false);
        assert!(
            register_gate(&bytes, &changed, "graph-v1:sha256:fixture", &schemas(), 100).is_err()
        );
        assert!(register_gate(&bytes, &data, "graph-v1:sha256:changed", &schemas(), 100).is_err());
        let extra = BTreeMap::from([
            ("result".into(), ValueType::Probability),
            ("other".into(), ValueType::Boolean),
        ]);
        assert!(register_gate(&bytes, &data, "graph-v1:sha256:fixture", &extra, 100).is_err());
        let duplicate = String::from_utf8(bytes).unwrap().replacen(
            "\"outputs\":{",
            &format!(
                "\"outputs\":{{\"result\":{},",
                serde_json::to_string(&bars()).unwrap()
            ),
            1,
        );
        assert!(serde_json::from_str::<PromotionGate>(&duplicate).is_err());
        let mut development = registered.gate.clone();
        development.split = Split::Development;
        assert!(development.validate().is_err());
        let mut invalid = registered.gate.clone();
        invalid
            .outputs
            .get_mut("result")
            .unwrap()
            .minimum_true_recall = 0.0;
        assert!(invalid.validate().is_err());
        let mut unknown = serde_json::to_value(&registered.gate).unwrap();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("model".into(), Json::String("forbidden".into()));
        assert!(serde_json::from_value::<PromotionGate>(unknown).is_err());
        assert!(
            serde_json::to_value(&registered.gate)
                .unwrap()
                .get("model")
                .is_none()
        );
    }
    /// Trace: FR-070-AC-1, FR-070-AC-2, FR-070-AC-3, FR-070-AC-4, FR-070-AC-5, FR-070-AC-6, FR-070-AC-7, IT-012-SC-02, IT-012-SC-03, IT-012-SC-04
    #[test]
    fn all_six_bars_include_primary_and_keep_partial_evidence() {
        let data = fixture(&[true, false, true, false]);
        let receipt = register_gate(
            &gate(&data, bars()),
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            100,
        )
        .unwrap();
        let primary = measured(&data, &[0.9, 0.1, 0.8, 0.2]);
        let changed = measured(&data, &[0.9, 0.1, 0.3, 0.2]);
        let report = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &primary,
            &[changed.clone(), changed],
            100,
        )
        .unwrap();
        assert!(report.complete);
        assert_eq!(report.outputs["result"].disagreement.observed, Some(0.25));
        assert_eq!(
            report.outputs["result"].disagreement.status,
            BarStatus::Pass
        );
        assert_eq!(report.verdict, GateVerdict::Go);
        let mut tighter = bars();
        tighter.maximum_disagreement = 0.0;
        let receipt = register_gate(
            &gate(&data, tighter),
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            100,
        )
        .unwrap();
        let failed = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &primary,
            &[primary.clone(), primary.clone()],
            100,
        )
        .unwrap();
        assert_eq!(failed.verdict, GateVerdict::Go);
        let changed = measured(&data, &[0.9, 0.1, 0.3, 0.2]);
        let failed = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &primary,
            &[changed.clone(), changed],
            100,
        )
        .unwrap();
        assert_eq!(
            failed.outputs["result"].disagreement.status,
            BarStatus::Fail
        );
        assert_eq!(failed.verdict, GateVerdict::NoGo);
        let tied = measured(&data, &[0.9, 0.9, 0.9, 0.9]);
        let tied_report = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &tied,
            &[tied.clone(), tied.clone()],
            100,
        )
        .unwrap();
        assert_eq!(tied_report.outputs["result"].margin.observed, Some(0.0));
        assert_eq!(tied_report.outputs["result"].margin.status, BarStatus::Fail);
        assert_eq!(
            tied_report.outputs["result"].false_recall.observed,
            Some(0.0)
        );
        let incomplete = measured(&data, &[0.9, 0.1, 0.8]);
        let partial = evaluate_gate(
            &receipt,
            &data,
            "graph-v1:sha256:fixture",
            &schemas(),
            &primary,
            &[incomplete.clone(), incomplete],
            100,
        )
        .unwrap();
        assert!(!partial.complete);
        assert_eq!(
            partial.outputs["result"].disagreement.status,
            BarStatus::NotComputed
        );
        assert_eq!(partial.outputs["result"].margin.status, BarStatus::Pass);
        assert_eq!(partial.verdict, GateVerdict::NoGo);
        let mut missing_label = data.clone();
        missing_label.cases[0].labels = BTreeMap::from([("other".into(), true)]);
        let receipt = register_gate(
            &gate(&missing_label, bars()),
            &missing_label,
            "graph-v1:sha256:fixture",
            &schemas(),
            100,
        )
        .unwrap();
        let partial_primary = measured(&missing_label, &[0.9, 0.1, 0.8, 0.2]);
        let missing = evaluate_gate(
            &receipt,
            &missing_label,
            "graph-v1:sha256:fixture",
            &schemas(),
            &partial_primary,
            &[partial_primary.clone(), partial_primary.clone()],
            100,
        )
        .unwrap();
        assert!(!missing.complete);
        assert_eq!(missing.verdict, GateVerdict::NoGo);
    }
}
