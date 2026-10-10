// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Deterministic, pure labelled slice and caller-defined window reporting (FR-067).
use crate::{Case, CaseOutcome, Dataset, EvidenceError, Measurement, Metrics, Split, measure};
use sapho_core::{ItemId, Value, ValueType, validate_name};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Missing path and literal text are different slice identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SliceKey {
    /// Path absent or explicit Optional absence.
    Missing,
    /// Exact nonblank Text value, including the literal word `missing`.
    Value(String),
}
/// One case's explicit reference/current membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowAssignment {
    /// Selected case identity.
    pub case: ItemId,
    /// One of the two caller-declared window names.
    pub window: String,
}
/// Caller-supplied window names and one assignment per selected case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    /// Earlier comparison population name, without inferred time semantics.
    pub reference: String,
    /// Later comparison population name, without inferred time semantics.
    pub current: String,
    /// Explicit selected-case membership, retaining duplicates for validation.
    pub assignments: Vec<WindowAssignment>,
}
/// Signed metric difference or a reason no difference is computable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaValue {
    /// Left minus right, when both metrics exist.
    pub value: Option<f64>,
    /// `metric_unavailable` or `missing_window_slice` when absent.
    pub absent_reason: Option<String>,
}
impl DeltaValue {
    fn between(left: Option<f64>, right: Option<f64>, missing_slice: bool) -> Self {
        let value = left.zip(right).map(|(a, b)| a - b);
        Self {
            value,
            absent_reason: value.is_none().then(|| {
                if missing_slice {
                    "missing_window_slice"
                } else {
                    "metric_unavailable"
                }
                .into()
            }),
        }
    }
}
/// Signed risk-table differences at one shared inclusive threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskDeltas {
    /// The shared threshold.
    pub threshold: f64,
    /// Left minus right answered fraction.
    pub coverage: DeltaValue,
    /// Left minus right error fraction, absent when either has no answers.
    pub risk: DeltaValue,
}
/// Per-output signed differences in available score families.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricDeltas {
    /// Boolean agreement difference.
    pub agreement: DeltaValue,
    /// Probability Brier difference.
    pub brier: DeltaValue,
    /// Shared FR-065 ECE difference.
    pub ece: DeltaValue,
    /// Shared FR-066 risk rows, sorted by threshold.
    pub risk_coverage: Vec<RiskDeltas>,
}
/// One measured slice and its signed difference from the selected overall population.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SliceEntry {
    /// Explicit slice identity.
    pub key: SliceKey,
    /// Selected case count, including unscored and self-source cases.
    pub selected_cases: u32,
    /// True when selected count is below the caller's positive minimum.
    pub too_small: bool,
    /// Ordinary measurement on exactly this slice's selected case IDs.
    pub measurement: Measurement,
    /// `slice - overall` metrics by named output.
    pub deltas: BTreeMap<String, MetricDeltas>,
}
/// One slice across both explicitly assigned windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSlice {
    /// Slice identity, ordered missing then exact text.
    pub key: SliceKey,
    /// Reference selected case count, including zero for an absent slice.
    pub reference_count: u32,
    /// Current selected case count, including zero for an absent slice.
    pub current_count: u32,
    /// Reference measurement when that slice has selected cases.
    pub reference: Option<Measurement>,
    /// Current measurement when that slice has selected cases.
    pub current: Option<Measurement>,
    /// `current - reference` metrics by named output.
    pub deltas: BTreeMap<String, MetricDeltas>,
}
/// Overall and per-slice measurements for two named caller-supplied windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowReport {
    /// Reference window name.
    pub reference_name: String,
    /// Current window name.
    pub current_name: String,
    /// Reference overall measurement.
    pub reference: Measurement,
    /// Current overall measurement.
    pub current: Measurement,
    /// `current - reference` for overall named outputs.
    pub deltas: BTreeMap<String, MetricDeltas>,
    /// Per-slice window comparisons in deterministic key order.
    pub slices: Vec<WindowSlice>,
}
/// Versioned, deterministic slice measurement without a Dataset schema change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SliceReport {
    /// Report schema version.
    pub version: u32,
    /// Caller-declared input and Record-field path.
    pub selector: Vec<String>,
    /// Selected Dataset split.
    pub split: Split,
    /// Positive minimum selected-case count.
    pub minimum_count: u32,
    /// Ordinary measurement on all selected cases.
    pub overall: Measurement,
    /// Missing first, then literal text values in UTF-8 order.
    pub slices: Vec<SliceEntry>,
    /// Optional explicit reference/current comparison.
    pub windows: Option<WindowReport>,
}

fn resolve_slice(case: &Case, selector: &[String]) -> Result<SliceKey, EvidenceError> {
    let path = selector.join(".");
    let Some(first) = selector.first() else {
        return Err(EvidenceError::InvalidSliceSelector);
    };
    let mut value = case.inputs.get(first).map(|datum| &datum.value);
    for member in selector.iter().skip(1) {
        value = match value {
            None | Some(Value::Optional(None)) => None,
            Some(Value::Optional(Some(inner))) => match inner.as_ref() {
                Value::Record(fields) => fields.get(member),
                _ => {
                    return Err(EvidenceError::InvalidSliceValue {
                        case: case.id.clone(),
                        path,
                    });
                }
            },
            Some(Value::Record(fields)) => fields.get(member),
            _ => {
                return Err(EvidenceError::InvalidSliceValue {
                    case: case.id.clone(),
                    path,
                });
            }
        };
    }
    match value {
        None | Some(Value::Optional(None)) => Ok(SliceKey::Missing),
        Some(Value::Text(text)) if !text.trim().is_empty() => Ok(SliceKey::Value(text.clone())),
        Some(Value::Optional(Some(inner))) => match inner.as_ref() {
            Value::Text(text) if !text.trim().is_empty() => Ok(SliceKey::Value(text.clone())),
            _ => Err(EvidenceError::InvalidSliceValue {
                case: case.id.clone(),
                path,
            }),
        },
        _ => Err(EvidenceError::InvalidSliceValue {
            case: case.id.clone(),
            path,
        }),
    }
}

fn measured_subset(
    dataset: &Dataset,
    split: Split,
    schemas: &BTreeMap<String, ValueType>,
    outcomes: &BTreeMap<ItemId, CaseOutcome>,
    cases: &[&Case],
    max_cases: usize,
) -> Result<Measurement, EvidenceError> {
    let mut selected = cases.iter().map(|case| (*case).clone()).collect::<Vec<_>>();
    selected.sort_by(|a, b| a.id.cmp(&b.id));
    measure(
        &Dataset {
            id: dataset.id.clone(),
            cases: selected,
        },
        split,
        schemas,
        outcomes,
        max_cases,
    )
}

fn partition<'a>(
    name: &str,
    cases: &[&'a Case],
    membership: &BTreeMap<ItemId, &str>,
) -> Vec<&'a Case> {
    cases
        .iter()
        .copied()
        .filter(|case| membership.get(&case.id).copied() == Some(name))
        .collect()
}

fn output_metrics<'a>(report: Option<&'a Measurement>, name: &str) -> Option<&'a Metrics> {
    report
        .and_then(|report| report.outputs.get(name))
        .map(|output| &output.metrics)
}

fn risk_rows(metrics: Option<&Metrics>) -> &[crate::RiskCoverageRow] {
    match metrics {
        Some(Metrics::Probability { risk_coverage, .. }) => risk_coverage,
        _ => &[],
    }
}

fn metric_deltas(
    left: Option<&Measurement>,
    right: Option<&Measurement>,
    outputs: &BTreeSet<String>,
    missing_slice: bool,
) -> BTreeMap<String, MetricDeltas> {
    outputs
        .iter()
        .map(|name| {
            let left = output_metrics(left, name);
            let right = output_metrics(right, name);
            let agreement = |metrics: Option<&Metrics>| match metrics {
                Some(Metrics::Boolean { agreement, .. }) => *agreement,
                _ => None,
            };
            let brier = |metrics: Option<&Metrics>| match metrics {
                Some(Metrics::Probability { brier, .. }) => *brier,
                _ => None,
            };
            let ece = |metrics: Option<&Metrics>| match metrics {
                Some(Metrics::Probability { ece, .. }) => *ece,
                _ => None,
            };
            let left_risks = risk_rows(left);
            let right_risks = risk_rows(right);
            let present_risks = if left_risks.is_empty() {
                right_risks
            } else {
                left_risks
            };
            let risk_coverage = present_risks
                .iter()
                .map(|row| {
                    let left_row = left_risks
                        .iter()
                        .find(|other| other.threshold == row.threshold);
                    let right_row = right_risks
                        .iter()
                        .find(|other| other.threshold == row.threshold);
                    RiskDeltas {
                        threshold: row.threshold,
                        coverage: DeltaValue::between(
                            left_row.map(|r| r.coverage),
                            right_row.map(|r| r.coverage),
                            missing_slice,
                        ),
                        risk: DeltaValue::between(
                            left_row.and_then(|r| r.risk),
                            right_row.and_then(|r| r.risk),
                            missing_slice,
                        ),
                    }
                })
                .collect();
            (
                name.clone(),
                MetricDeltas {
                    agreement: DeltaValue::between(
                        agreement(left),
                        agreement(right),
                        missing_slice,
                    ),
                    brier: DeltaValue::between(brier(left), brier(right), missing_slice),
                    ece: DeltaValue::between(ece(left), ece(right), missing_slice),
                    risk_coverage,
                },
            )
        })
        .collect()
}

/// Partition selected cases by a typed input path and optionally compare explicit windows.
pub fn report_slices(
    dataset: &Dataset,
    split: Split,
    schemas: &BTreeMap<String, ValueType>,
    outcomes: &BTreeMap<ItemId, CaseOutcome>,
    selector: &[String],
    minimum_count: usize,
    windows: Option<&WindowSpec>,
    max_cases: usize,
) -> Result<SliceReport, EvidenceError> {
    dataset.validate(max_cases)?;
    if selector.is_empty()
        || selector.iter().any(|part| validate_name(part).is_err())
        || minimum_count == 0
        || u32::try_from(minimum_count).is_err()
    {
        return Err(EvidenceError::InvalidSliceSelector);
    }
    let selected = dataset.selected(split).collect::<Vec<_>>();
    let mut slices = BTreeMap::<SliceKey, Vec<&Case>>::new();
    for case in &selected {
        slices
            .entry(resolve_slice(case, selector)?)
            .or_default()
            .push(case);
    }
    let membership = if let Some(spec) = windows {
        if spec.reference == spec.current
            || spec.reference.trim().is_empty()
            || spec.current.trim().is_empty()
        {
            return Err(EvidenceError::InvalidWindowMembership);
        }
        let selected_ids = selected
            .iter()
            .map(|case| &case.id)
            .collect::<BTreeSet<_>>();
        let mut assigned = BTreeMap::new();
        for assignment in &spec.assignments {
            if !selected_ids.contains(&assignment.case)
                || (assignment.window != spec.reference && assignment.window != spec.current)
                || assigned
                    .insert(assignment.case.clone(), assignment.window.as_str())
                    .is_some()
            {
                return Err(EvidenceError::InvalidWindowMembership);
            }
        }
        if assigned.len() != selected.len() {
            return Err(EvidenceError::InvalidWindowMembership);
        }
        Some(assigned)
    } else {
        None
    };
    let overall = measured_subset(dataset, split, schemas, outcomes, &selected, max_cases)?;
    let outputs = overall.outputs.keys().cloned().collect::<BTreeSet<_>>();
    let entries = slices
        .iter()
        .map(|(key, cases)| {
            let measurement = measured_subset(dataset, split, schemas, outcomes, cases, max_cases)?;
            Ok(SliceEntry {
                key: key.clone(),
                selected_cases: measurement.selected_cases,
                too_small: cases.len() < minimum_count,
                deltas: metric_deltas(Some(&measurement), Some(&overall), &outputs, false),
                measurement,
            })
        })
        .collect::<Result<Vec<_>, EvidenceError>>()?;
    let windows = if let (Some(spec), Some(membership)) = (windows, membership) {
        let reference_cases = partition(&spec.reference, &selected, &membership);
        let current_cases = partition(&spec.current, &selected, &membership);
        let reference = measured_subset(
            dataset,
            split,
            schemas,
            outcomes,
            &reference_cases,
            max_cases,
        )?;
        let current =
            measured_subset(dataset, split, schemas, outcomes, &current_cases, max_cases)?;
        let mut window_slices = Vec::new();
        for (key, cases) in &slices {
            let reference_cases = partition(&spec.reference, cases, &membership);
            let current_cases = partition(&spec.current, cases, &membership);
            let reference_slice = (!reference_cases.is_empty())
                .then(|| {
                    measured_subset(
                        dataset,
                        split,
                        schemas,
                        outcomes,
                        &reference_cases,
                        max_cases,
                    )
                })
                .transpose()?;
            let current_slice = (!current_cases.is_empty())
                .then(|| {
                    measured_subset(dataset, split, schemas, outcomes, &current_cases, max_cases)
                })
                .transpose()?;
            window_slices.push(WindowSlice {
                key: key.clone(),
                reference_count: u32::try_from(reference_cases.len()).unwrap_or(u32::MAX),
                current_count: u32::try_from(current_cases.len()).unwrap_or(u32::MAX),
                deltas: metric_deltas(
                    current_slice.as_ref(),
                    reference_slice.as_ref(),
                    &outputs,
                    reference_slice.is_none() || current_slice.is_none(),
                ),
                reference: reference_slice,
                current: current_slice,
            });
        }
        Some(WindowReport {
            reference_name: spec.reference.clone(),
            current_name: spec.current.clone(),
            deltas: metric_deltas(Some(&current), Some(&reference), &outputs, false),
            reference,
            current,
            slices: window_slices,
        })
    } else {
        None
    };
    Ok(SliceReport {
        version: 1,
        selector: selector.to_vec(),
        split,
        minimum_count: u32::try_from(minimum_count).unwrap_or(u32::MAX),
        overall,
        slices: entries,
        windows,
    })
}
