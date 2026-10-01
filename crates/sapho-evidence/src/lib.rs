// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure evidence contracts, scoring and ranking; no execution, filesystem or models.
use sapho_core::{
    ErrorCode, Inputs, ItemId, SaphoError, SourceId, Value, ValueType, bounded_json, validate_name,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Refusals distinguish dataset, coverage and candidate failures at this crate boundary.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum EvidenceError {
    /// Shared value or serialization refusal.
    #[error(transparent)]
    Core(#[from] SaphoError),
    /// Two cases have the same identity.
    #[error("Duplicate case {0}")]
    DuplicateCase(ItemId),
    /// A case has no declared supervision.
    #[error("Missing labels for {0}")]
    MissingLabels(ItemId),
    /// Label origin is absent.
    #[error("Missing label provenance for {0}")]
    MissingProvenance(ItemId),
    /// A finite case ceiling was exceeded.
    #[error("Dataset exceeds {limit} cases")]
    CaseLimit {
        /// Explicit ceiling.
        limit: usize,
    },
    /// No cases belong to the requested split.
    #[error("No cases in {0:?}")]
    EmptySplit(Split),
    /// No candidate was supplied.
    #[error("No candidate graphs supplied")]
    NoCandidates,
    /// Candidate count exceeds its explicit ceiling.
    #[error("Candidate count exceeds {limit}")]
    CandidateLimit {
        /// Explicit ceiling.
        limit: usize,
    },
    /// Coverage or metric incompatibility leaves no winner.
    #[error("No completely scored candidate for {output}")]
    NoRankableCandidate {
        /// Selected output.
        output: String,
    },
}
/// Explicit development/held-out partition; tuning and exports always select development.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Cases used to author and tune graphs.
    Development,
    /// Cases reserved for a separately requested evaluation.
    HeldOut,
}
/// One independently curated case; model recordings cannot deserialize into this schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Original case identity.
    pub id: ItemId,
    /// Declared partition.
    pub split: Split,
    /// Existing typed engine input representation.
    pub inputs: Inputs,
    /// Supplied Boolean outcomes by named graph output.
    pub labels: BTreeMap<String, bool>,
    /// Caller-declared label origin/reference, not a certification of truth.
    pub label_provenance: String,
}
/// Labelled data, separate from recordings and traces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    /// Caller-owned identity for this curated collection.
    pub id: SourceId,
    /// Ordered identified cases.
    pub cases: Vec<Case>,
}
impl Dataset {
    /// Check identities, supervision, attribution and the declared case ceiling.
    pub fn validate(&self, max_cases: usize) -> Result<(), EvidenceError> {
        self.id.validate()?;
        if self.cases.len() > max_cases
            || max_cases == 0
            || u32::try_from(self.cases.len()).is_err()
        {
            return Err(EvidenceError::CaseLimit { limit: max_cases });
        }
        let mut seen = BTreeSet::new();
        for case in &self.cases {
            case.id.validate()?;
            if !seen.insert(&case.id) {
                return Err(EvidenceError::DuplicateCase(case.id.clone()));
            }
            if case.labels.is_empty() {
                return Err(EvidenceError::MissingLabels(case.id.clone()));
            }
            if case.label_provenance.trim().is_empty() {
                return Err(EvidenceError::MissingProvenance(case.id.clone()));
            }
            for name in case.labels.keys() {
                validate_name(name)?;
            }
            for (name, datum) in &case.inputs {
                validate_name(name)?;
                datum.validate()?;
            }
        }
        Ok(())
    }
    /// Borrow only the chosen partition; this performs no graph work.
    pub fn selected(&self, split: Split) -> impl Iterator<Item = &Case> {
        self.cases.iter().filter(move |case| case.split == split)
    }
}
/// Outcome supplied by a command host after actual bounded execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaseOutcome {
    /// Checked named outputs.
    Completed {
        /// Outputs returned by the engine.
        outputs: Inputs,
    },
    /// Execution refusal, without inventing a prediction.
    Failed {
        /// Stable engine error.
        error: SaphoError,
    },
}
/// Boolean counts against supplied labels, never an author-reported accuracy claim.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confusion {
    /// Predicted true, label true.
    pub true_positive: u32,
    /// Predicted false, label false.
    pub true_negative: u32,
    /// Predicted true, label false.
    pub false_positive: u32,
    /// Predicted false, label true.
    pub false_negative: u32,
}
/// A metric has its own semantic type and explicit scored denominator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Metrics {
    /// Crisp agreement with supplied labels.
    Boolean {
        /// Exact counts.
        confusion: Confusion,
        /// Fraction agreeing; absent with no scored cases.
        agreement: Option<f64>,
    },
    /// Mean squared probability error against Boolean labels.
    Probability {
        /// Brier mean; absent with no scored cases.
        brier: Option<f64>,
    },
    /// A missing or unsupported output cannot be scored as a probability.
    Unsupported {
        /// Declared output schema, when present.
        value_type: Option<ValueType>,
    },
}
/// Coverage and metrics for one output; different outputs are never averaged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputMeasurement {
    /// Selected labelled cases for this output.
    pub labelled: u32,
    /// Predictions successfully scored.
    pub scored: u32,
    /// Unsupported/missing predictions.
    pub unscored: u32,
    /// Execution failures.
    pub failed: u32,
    /// Type-specific metric.
    pub metrics: Metrics,
}
/// Explanation for an unscored label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnscoredReason {
    /// No output of the requested name.
    MissingOutput,
    /// The output schema is not Boolean or Probability.
    UnsupportedType,
    /// Returned data differ from the declaration.
    TypeMismatch,
}
/// Retained per-case/output evidence with label origin and observed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prediction {
    /// Original case identity.
    pub case: ItemId,
    /// Named output being scored.
    pub output: String,
    /// Supplied Boolean label.
    pub label: bool,
    /// Caller-declared origin.
    pub label_provenance: String,
    /// Actual value, including unsupported values, when available.
    pub predicted: Option<Value>,
    /// Explicit reason for missing coverage.
    pub unscored: Option<UnscoredReason>,
    /// Execution failure when present.
    pub error: Option<SaphoError>,
}
/// Complete agreement report against supplied labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// Dataset identity.
    pub dataset: SourceId,
    /// Explicitly selected partition.
    pub split: Split,
    /// Total cases in this partition, irrespective of output label membership.
    pub selected_cases: u32,
    /// Separate coverage and metrics for each output.
    pub outputs: BTreeMap<String, OutputMeasurement>,
    /// Observed values, labels and failures.
    pub predictions: Vec<Prediction>,
}
impl Measurement {
    /// True only when every selected label was scored without failure.
    pub fn complete(&self) -> bool {
        self.outputs
            .values()
            .all(|o| o.failed == 0 && o.unscored == 0)
    }
}
/// Score real host outcomes against the selected labelled partition.
pub fn measure(
    dataset: &Dataset,
    split: Split,
    schemas: &BTreeMap<String, ValueType>,
    outcomes: &BTreeMap<ItemId, CaseOutcome>,
    max_cases: usize,
) -> Result<Measurement, EvidenceError> {
    dataset.validate(max_cases)?;
    for schema in schemas.values() {
        schema.validate()?;
    }
    let mut report = Measurement {
        dataset: dataset.id.clone(),
        split,
        selected_cases: 0,
        outputs: BTreeMap::new(),
        predictions: Vec::new(),
    };
    let mut squared_errors = BTreeMap::<String, f64>::new();
    for case in dataset.selected(split) {
        report.selected_cases += 1;
        for (name, label) in &case.labels {
            let schema = schemas.get(name);
            let counts = report
                .outputs
                .entry(name.clone())
                .or_insert_with(|| OutputMeasurement {
                    labelled: 0,
                    scored: 0,
                    unscored: 0,
                    failed: 0,
                    metrics: match schema {
                        Some(ValueType::Boolean) => Metrics::Boolean {
                            confusion: Confusion::default(),
                            agreement: None,
                        },
                        Some(ValueType::Probability) => Metrics::Probability { brier: None },
                        other => Metrics::Unsupported {
                            value_type: other.cloned(),
                        },
                    },
                });
            counts.labelled += 1;
            let mut prediction = Prediction {
                case: case.id.clone(),
                output: name.clone(),
                label: *label,
                label_provenance: case.label_provenance.clone(),
                predicted: None,
                unscored: None,
                error: None,
            };
            match outcomes.get(&case.id) {
                Some(CaseOutcome::Failed { error }) => {
                    counts.failed += 1;
                    prediction.error = Some(error.clone());
                }
                None => {
                    counts.failed += 1;
                    prediction.error = Some(SaphoError::new(
                        ErrorCode::MissingInput,
                        "Case outcome absent",
                    ));
                }
                Some(CaseOutcome::Completed { outputs }) => {
                    if let Some(datum) = outputs.get(name) {
                        datum.validate()?;
                        prediction.predicted = Some(datum.value.clone());
                        match (&mut counts.metrics, &datum.value) {
                            (Metrics::Boolean { confusion, .. }, Value::Boolean(value)) => {
                                counts.scored += 1;
                                match (*value, *label) {
                                    (true, true) => confusion.true_positive += 1,
                                    (false, false) => confusion.true_negative += 1,
                                    (true, false) => confusion.false_positive += 1,
                                    (false, true) => confusion.false_negative += 1,
                                }
                            }
                            (Metrics::Probability { .. }, Value::Probability(value)) => {
                                counts.scored += 1;
                                let delta = value.get() - if *label { 1.0 } else { 0.0 };
                                *squared_errors.entry(name.clone()).or_default() += delta * delta;
                            }
                            (Metrics::Unsupported { .. }, _) => {
                                counts.unscored += 1;
                                prediction.unscored = Some(UnscoredReason::UnsupportedType);
                            }
                            (Metrics::Boolean { .. } | Metrics::Probability { .. }, _) => {
                                counts.unscored += 1;
                                prediction.unscored = Some(UnscoredReason::TypeMismatch);
                            }
                        }
                    } else {
                        counts.unscored += 1;
                        prediction.unscored = Some(UnscoredReason::MissingOutput);
                    }
                }
            }
            report.predictions.push(prediction);
        }
    }
    for (name, counts) in &mut report.outputs {
        if counts.scored == 0 {
            continue;
        }
        match &mut counts.metrics {
            Metrics::Boolean {
                confusion,
                agreement,
            } => {
                *agreement = Some(
                    f64::from(confusion.true_positive + confusion.true_negative)
                        / f64::from(counts.scored),
                )
            }
            Metrics::Probability { brier } => {
                *brier = squared_errors
                    .get(name)
                    .map(|sum| sum / f64::from(counts.scored))
            }
            Metrics::Unsupported { .. } => {}
        }
    }
    Ok(report)
}
/// Metric selected explicitly for candidate comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Maximize Boolean agreement.
    Agreement,
    /// Minimize mean squared probability error.
    Brier,
}
/// One candidate's measured development evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// Original candidate identity; the command host retains its definition separately.
    pub id: SourceId,
    /// Real measured development report.
    pub measurement: Measurement,
}
/// Stable ranking of completely scored candidates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankedCandidate {
    /// Candidate identity.
    pub id: SourceId,
    /// Zero-based index in the supplied ordered candidate list.
    pub input_index: usize,
    /// Selected metric value.
    pub score: f64,
}
/// Rank only complete development measurements; ties preserve input order.
pub fn rank(
    candidates: &[Candidate],
    output: &str,
    metric: Metric,
    max_candidates: usize,
) -> Result<Vec<RankedCandidate>, EvidenceError> {
    if candidates.is_empty() {
        return Err(EvidenceError::NoCandidates);
    }
    if candidates.len() > max_candidates {
        return Err(EvidenceError::CandidateLimit {
            limit: max_candidates,
        });
    }
    let mut ranked = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.measurement.split != Split::Development
            || candidate.measurement.selected_cases == 0
        {
            continue;
        }
        let Some(o) = candidate.measurement.outputs.get(output) else {
            continue;
        };
        // Every development case must carry and score the selected label.
        if o.scored != candidate.measurement.selected_cases
            || o.labelled != o.scored
            || o.failed != 0
            || o.unscored != 0
        {
            continue;
        }
        let score = match (&o.metrics, metric) {
            (Metrics::Boolean { agreement, .. }, Metric::Agreement) => *agreement,
            (Metrics::Probability { brier }, Metric::Brier) => *brier,
            _ => None,
        };
        if let Some(score) = score.filter(|v| v.is_finite()) {
            ranked.push(RankedCandidate {
                id: candidate.id.clone(),
                input_index: index,
                score,
            });
        }
    }
    ranked.sort_by(|a, b| {
        let comparison = a.score.total_cmp(&b.score);
        if metric == Metric::Agreement {
            comparison.reverse()
        } else {
            comparison
        }
    });
    if ranked.is_empty() {
        return Err(EvidenceError::NoRankableCandidate {
            output: output.into(),
        });
    }
    Ok(ranked)
}
/// Model-independent curated supervision record; downstream trainers own conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingRow {
    /// Original dataset identity.
    pub dataset: SourceId,
    /// Original case identity.
    pub id: ItemId,
    /// Original typed inputs and sources.
    pub inputs: Inputs,
    /// Explicit supplied labels.
    pub labels: BTreeMap<String, bool>,
    /// Label origin declaration.
    pub label_provenance: String,
}
/// Export development supervision as bounded JSONL bytes; no I/O or model work.
pub fn export_training(
    dataset: &Dataset,
    max_cases: usize,
    max_bytes: usize,
) -> Result<Vec<u8>, EvidenceError> {
    dataset.validate(max_cases)?;
    let mut bytes = Vec::new();
    for case in dataset.selected(Split::Development) {
        let row = TrainingRow {
            dataset: dataset.id.clone(),
            id: case.id.clone(),
            inputs: case.inputs.clone(),
            labels: case.labels.clone(),
            label_provenance: case.label_provenance.clone(),
        };
        let remaining = max_bytes
            .checked_sub(bytes.len())
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| {
                SaphoError::new(
                    ErrorCode::LimitExceeded,
                    "Training export exceeds byte ceiling",
                )
            })?;
        let line = bounded_json(&row, remaining)?;
        bytes.extend(line);
        bytes.push(b'\n');
    }
    if bytes.is_empty() {
        return Err(EvidenceError::EmptySplit(Split::Development));
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests;
