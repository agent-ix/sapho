// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure, attributable model roster calculation and graph literal projection (FR-062/063).
use crate::{
    Case, CaseOutcome, Dataset, EvidenceError, LabelKind, OutputMeasurement, Split, measure,
};
use sapho_core::{
    BackendId, Datum, ItemId, ProviderDescriptor, SourceId, Usage, Value, ValueType, validate_name,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The declared question family for an output mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RosterQuestionKind {
    /// Two outcome question.
    Boolean,
    /// Named choice question.
    Choice,
    /// Ordered score question.
    Score,
}
/// One author-declared output attribution target, without supplied metric values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterMapping {
    /// Backend whose response directly contributes to the output.
    pub binding: BackendId,
    /// Declared question family.
    pub question_kind: RosterQuestionKind,
}
/// Successful response found in the transitive dependency lineage of one output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterContributor {
    /// Actual binding of the response.
    pub binding: BackendId,
    /// Actual reported model identity, when present.
    pub actual_model: Option<String>,
    /// Question family of the contributing request, absent when its questions mix families.
    pub question_kind: Option<RosterQuestionKind>,
}
/// Whether a call used live inference or exact recording replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RosterMode {
    /// Live backend calls.
    Live,
    /// Exact replay calls.
    Replay,
}
/// Pure call projection from the runtime observation sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterCall {
    /// Trace node path.
    pub path: Vec<String>,
    /// Backend binding.
    pub binding: BackendId,
    /// Actual response model, absent on response-less attempts.
    pub actual_model: Option<String>,
    /// Whether the backend response passed the model node's validation.
    pub completed: bool,
    /// Usage supplied by a response.
    pub usage: Option<Usage>,
    /// Live monotonic duration in microseconds.
    pub elapsed_micros: Option<u64>,
    /// Validated nonsecret provider information.
    pub descriptor: Option<ProviderDescriptor>,
    /// Explicit inference mode.
    pub mode: RosterMode,
}
/// One selected case's outcome, calls and output-to-response lineage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterCase {
    /// Existing decision outcome.
    pub outcome: CaseOutcome,
    /// Every attempted model call.
    pub calls: Vec<RosterCall>,
    /// Successful contributing responses by named output.
    pub lineage: BTreeMap<String, Vec<RosterContributor>>,
}
/// A total, denominator and mean for reported provider quantities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterTotal {
    /// Sum of reported values.
    pub total: u64,
    /// Number of values contributing to the sum.
    pub count: u32,
    /// Mean over contributors only.
    pub mean: f64,
}
/// Live latency statistics; absent fields identify replay-only data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterLatency {
    /// Number of measured calls.
    pub count: u32,
    /// Arithmetic mean in microseconds.
    pub mean_micros: Option<f64>,
    /// Nearest-rank median in microseconds.
    pub p50_micros: Option<u64>,
    /// Nearest-rank 95th percentile in microseconds.
    pub p95_micros: Option<u64>,
    /// `replay_only` when no live durations exist.
    pub absent_reason: Option<String>,
}
/// Measured cases for one output and declared label kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterMetric {
    /// Case IDs contributing to this entry's denominator scope.
    pub case_ids: Vec<ItemId>,
    /// Existing measure counts and typed score on exactly these cases.
    pub measurement: OutputMeasurement,
    /// Shared calibration metric availability.
    pub ece: Option<f64>,
    /// `not_computed` until SAPHO-21 provides the shared metric.
    pub ece_absent_reason: Option<String>,
}
/// One actual model under a binding, or its response-less bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterEntry {
    /// Declared binding identity.
    pub binding: BackendId,
    /// Reported actual model; `None` is the explicit unknown bucket.
    pub actual_model: Option<String>,
    /// Safe descriptor, if supplied.
    pub descriptor: Option<ProviderDescriptor>,
    /// Attempted call count.
    pub calls: u32,
    /// Reported input-token aggregate.
    pub input_tokens: Option<RosterTotal>,
    /// Reported output-token aggregate.
    pub output_tokens: Option<RosterTotal>,
    /// Reported billing-unit aggregate.
    pub billing_units: Option<RosterTotal>,
    /// Latency statistics over observed live attempts.
    pub latency: RosterLatency,
    /// Metrics keyed by public output and label kind.
    pub outputs: BTreeMap<String, BTreeMap<LabelKind, RosterMetric>>,
}
/// Why a labelled output could not be assigned to one actual model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnattributedReason {
    /// No mapping was declared.
    MissingMapping,
    /// No successful contributing response exists.
    NoResponse,
    /// More than one response contributes.
    MultipleResponses,
    /// The response used another binding.
    WrongBinding,
    /// The response did not report an actual model.
    UnknownActualModel,
    /// No case outcome was supplied.
    MissingOutcome,
}
/// Explicitly unassigned case/output evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnattributedCase {
    /// Case identity.
    pub case: ItemId,
    /// Public output.
    pub output: String,
    /// Refusal reason.
    pub reason: UnattributedReason,
}
/// Versioned, split-specific model roster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roster {
    /// Wire schema version.
    pub version: u32,
    /// Curated dataset identity.
    pub dataset: SourceId,
    /// Explicit selected split.
    pub split: Split,
    /// Versioned semantic graph identity.
    pub graph_semantic_identity: String,
    /// Sapho build version.
    pub sapho_version: String,
    /// Explicit author-declared output attribution targets.
    pub mappings: BTreeMap<String, RosterMapping>,
    /// Sorted per-(binding, actual model) entries.
    pub entries: Vec<RosterEntry>,
    /// Labelled outputs with no unique attributable response.
    pub unattributed: Vec<UnattributedCase>,
}

/// Author-selected entry and graph field name for a measured literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterSelection {
    /// Graph Record field name.
    pub field: String,
    /// Binding identity of the selected entry.
    pub binding: BackendId,
    /// Reported actual model; absent selects the unknown bucket.
    pub actual_model: Option<String>,
}
/// Generated value and exact schema for an ordinary graph Binding::Literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterLiteral {
    /// Measured projection, with no caller-supplied metric fields.
    pub value: Datum,
    /// Exact Record type.
    pub value_type: ValueType,
}
fn optional_number(value: Option<f64>) -> Value {
    Value::Optional(value.map(|value| Box::new(Value::Number(value))))
}
fn optional_text(value: Option<&str>) -> Value {
    Value::Optional(value.map(|value| Box::new(Value::Text(value.into()))))
}
fn number_u64(value: u64) -> Result<f64, EvidenceError> {
    if value > (1u64 << 53) {
        return Err(EvidenceError::InvalidRoster(
            "Profile number exceeds exact f64 integer range".into(),
        ));
    }
    Ok(value as f64)
}
fn profile_value(
    entry: &RosterEntry,
    roster: &Roster,
) -> Result<(Value, ValueType), EvidenceError> {
    if let Some(descriptor) = &entry.descriptor {
        descriptor.validate()?;
    }
    let mut fields = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut put = |name: &str, value: Value, ty: ValueType| {
        fields.insert(name.into(), value);
        types.insert(name.into(), ty);
    };
    put(
        "binding",
        Value::Text(entry.binding.to_string()),
        ValueType::Text,
    );
    put(
        "actual_model",
        optional_text(entry.actual_model.as_deref()),
        ValueType::optional(ValueType::Text),
    );
    put(
        "provider",
        optional_text(
            entry
                .descriptor
                .as_ref()
                .and_then(|d| d.provider.as_deref()),
        ),
        ValueType::optional(ValueType::Text),
    );
    put(
        "adapter",
        optional_text(entry.descriptor.as_ref().and_then(|d| d.adapter.as_deref())),
        ValueType::optional(ValueType::Text),
    );
    put(
        "dataset",
        Value::Text(roster.dataset.to_string()),
        ValueType::Text,
    );
    put(
        "split",
        Value::Text(
            match roster.split {
                Split::Development => "development",
                Split::HeldOut => "held_out",
            }
            .into(),
        ),
        ValueType::Text,
    );
    put(
        "graph_semantic_identity",
        Value::Text(roster.graph_semantic_identity.clone()),
        ValueType::Text,
    );
    put(
        "sapho_version",
        Value::Text(roster.sapho_version.clone()),
        ValueType::Text,
    );
    put(
        "calls",
        Value::Number(f64::from(entry.calls)),
        ValueType::Number,
    );
    for (name, value) in [
        ("input_tokens", entry.input_tokens.as_ref().map(|v| v.total)),
        (
            "output_tokens",
            entry.output_tokens.as_ref().map(|v| v.total),
        ),
        (
            "billing_units",
            entry.billing_units.as_ref().map(|v| v.total),
        ),
    ] {
        put(
            name,
            optional_number(value.map(number_u64).transpose()?),
            ValueType::optional(ValueType::Number),
        );
    }
    for (name, value) in [
        ("latency_mean_micros", entry.latency.mean_micros),
        (
            "latency_p50_micros",
            entry.latency.p50_micros.map(number_u64).transpose()?,
        ),
        (
            "latency_p95_micros",
            entry.latency.p95_micros.map(number_u64).transpose()?,
        ),
    ] {
        put(
            name,
            optional_number(value),
            ValueType::optional(ValueType::Number),
        );
    }
    let mut output_values = BTreeMap::new();
    let mut output_types = BTreeMap::new();
    for (name, kinds) in &entry.outputs {
        validate_name(name)?;
        let mut kind_values = BTreeMap::new();
        let mut kind_types = BTreeMap::new();
        for (kind, metric) in kinds {
            let kind_name = match kind {
                LabelKind::Model => "model",
                LabelKind::Agent => "agent",
                LabelKind::Human => "human",
                LabelKind::DeterministicCheck => "deterministic_check",
            };
            let mut metric_values = BTreeMap::new();
            let mut metric_types = BTreeMap::new();
            metric_values.insert(
                "scored".into(),
                Value::Number(f64::from(metric.measurement.scored)),
            );
            metric_types.insert("scored".into(), ValueType::Number);
            let (agreement, brier) = match &metric.measurement.metrics {
                crate::Metrics::Boolean { agreement, .. } => (*agreement, None),
                crate::Metrics::Probability { brier } => (None, *brier),
                crate::Metrics::Unsupported { .. } => (None, None),
            };
            for (field, value) in [
                ("agreement", agreement),
                ("brier", brier),
                ("ece", metric.ece),
            ] {
                metric_values.insert(field.into(), optional_number(value));
                metric_types.insert(field.into(), ValueType::optional(ValueType::Number));
            }
            kind_values.insert(kind_name.into(), Value::Record(metric_values));
            kind_types.insert(
                kind_name.into(),
                ValueType::Record {
                    fields: metric_types,
                },
            );
        }
        output_values.insert(name.clone(), Value::Record(kind_values));
        output_types.insert(name.clone(), ValueType::Record { fields: kind_types });
    }
    put(
        "outputs",
        Value::Record(output_values),
        ValueType::Record {
            fields: output_types,
        },
    );
    Ok((Value::Record(fields), ValueType::Record { fields: types }))
}
/// Project only selected measured entries into a checked typed Record literal.
pub fn project_roster_literal(
    roster: &Roster,
    selections: &[RosterSelection],
    max_bytes: usize,
) -> Result<RosterLiteral, EvidenceError> {
    if roster.version != 1 || selections.is_empty() || selections.len() > 32 {
        return Err(EvidenceError::InvalidRoster(
            "Unsupported roster version or selection count".into(),
        ));
    }
    let mut values = BTreeMap::new();
    let mut types = BTreeMap::new();
    for selection in selections {
        validate_name(&selection.field)?;
        if values.contains_key(&selection.field) {
            return Err(EvidenceError::InvalidRoster(
                "Duplicate roster field".into(),
            ));
        }
        let entry = roster
            .entries
            .iter()
            .find(|entry| {
                entry.binding == selection.binding && entry.actual_model == selection.actual_model
            })
            .ok_or_else(|| EvidenceError::InvalidRoster("Unknown roster entry".into()))?;
        let (value, ty) = profile_value(entry, roster)?;
        values.insert(selection.field.clone(), value);
        types.insert(selection.field.clone(), ty);
    }
    let value_type = ValueType::Record { fields: types };
    let value = Datum::new("roster-projection", Value::Record(values))?;
    value_type.check(&value.value)?;
    sapho_core::bounded_json(&value, max_bytes)?;
    Ok(RosterLiteral { value, value_type })
}

#[derive(Default)]
struct Accumulator {
    calls: u32,
    descriptor: Option<ProviderDescriptor>,
    input: Vec<u64>,
    output: Vec<u64>,
    billing: Vec<u64>,
    durations: Vec<u64>,
    cases: BTreeMap<(String, LabelKind), BTreeSet<ItemId>>,
}
fn total(values: &[u64]) -> Result<Option<RosterTotal>, EvidenceError> {
    if values.is_empty() {
        return Ok(None);
    }
    let sum = values
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(*value))
        .ok_or_else(|| EvidenceError::InvalidRoster("Usage total overflow".into()))?;
    let count = u32::try_from(values.len())
        .map_err(|_| EvidenceError::InvalidRoster("Usage count overflow".into()))?;
    Ok(Some(RosterTotal {
        total: sum,
        count,
        mean: sum as f64 / f64::from(count),
    }))
}
fn latency(mut values: Vec<u64>) -> Result<RosterLatency, EvidenceError> {
    if values.is_empty() {
        return Ok(RosterLatency {
            count: 0,
            mean_micros: None,
            p50_micros: None,
            p95_micros: None,
            absent_reason: Some("replay_only".into()),
        });
    }
    values.sort_unstable();
    let count = u32::try_from(values.len())
        .map_err(|_| EvidenceError::InvalidRoster("Latency count overflow".into()))?;
    let sum = values
        .iter()
        .fold(0u128, |sum, value| sum + u128::from(*value));
    let rank = |percent: usize| values[(percent * values.len()).div_ceil(100).saturating_sub(1)];
    Ok(RosterLatency {
        count,
        mean_micros: Some(sum as f64 / f64::from(count)),
        p50_micros: Some(rank(50)),
        p95_micros: Some(rank(95)),
        absent_reason: None,
    })
}

/// Calculate profiles from bounded selected cases and explicit lineage.
pub fn roster(
    dataset: &Dataset,
    split: Split,
    graph_identity: &str,
    version: &str,
    schemas: &BTreeMap<String, ValueType>,
    mappings: &BTreeMap<String, RosterMapping>,
    cases: &BTreeMap<ItemId, RosterCase>,
    max_cases: usize,
) -> Result<Roster, EvidenceError> {
    dataset.validate(max_cases)?;
    if dataset.selected(split).next().is_none() {
        return Err(EvidenceError::EmptySplit(split));
    }
    if !graph_identity.starts_with("graph-v1:sha256:") || version.is_empty() {
        return Err(EvidenceError::InvalidRoster(
            "Invalid roster identity".into(),
        ));
    }
    for (output, mapping) in mappings {
        validate_name(output)?;
        mapping.binding.validate()?;
        if !schemas.contains_key(output) {
            return Err(EvidenceError::InvalidRoster(format!(
                "Unknown mapped output {output}"
            )));
        }
    }
    let mut entries = BTreeMap::<(BackendId, Option<String>), Accumulator>::new();
    let mut mode = None;
    let mut unattributed = Vec::new();
    for case in dataset.selected(split) {
        let Some(evidence) = cases.get(&case.id) else {
            for output in case.labels.keys() {
                unattributed.push(UnattributedCase {
                    case: case.id.clone(),
                    output: output.clone(),
                    reason: UnattributedReason::MissingOutcome,
                });
            }
            continue;
        };
        for call in &evidence.calls {
            call.binding.validate()?;
            if let Some(descriptor) = &call.descriptor {
                descriptor.validate()?;
            }
            if mode.is_some_and(|prior| prior != call.mode) {
                return Err(EvidenceError::InvalidRoster(
                    "Mixed live and replay observations".into(),
                ));
            }
            mode = Some(call.mode);
            if call.mode == RosterMode::Replay && call.elapsed_micros.is_some() {
                return Err(EvidenceError::InvalidRoster(
                    "Replay duration present".into(),
                ));
            }
            if call.mode == RosterMode::Live && call.elapsed_micros.is_none() {
                return Err(EvidenceError::InvalidRoster("Live duration absent".into()));
            }
            if call.actual_model.as_deref() == Some("") {
                return Err(EvidenceError::InvalidRoster("Empty actual model".into()));
            }
            let entry = entries
                .entry((call.binding.clone(), call.actual_model.clone()))
                .or_default();
            entry.calls = entry
                .calls
                .checked_add(1)
                .ok_or_else(|| EvidenceError::InvalidRoster("Call count overflow".into()))?;
            if entry.descriptor.is_some()
                && call.descriptor.is_some()
                && entry.descriptor != call.descriptor
            {
                return Err(EvidenceError::InvalidRoster(
                    "Conflicting provider descriptors".into(),
                ));
            }
            if entry.descriptor.is_none() {
                entry.descriptor = call.descriptor.clone();
            }
            if let Some(usage) = &call.usage {
                entry.input.push(usage.input_tokens);
                entry.output.push(usage.output_tokens);
                if let Some(units) = usage.billing_units {
                    entry.billing.push(units);
                }
            }
            if let Some(duration) = call.elapsed_micros {
                entry.durations.push(duration);
            }
        }
        for output in case.labels.keys() {
            let reason = match mappings.get(output) {
                None => Some(UnattributedReason::MissingMapping),
                Some(mapping) => {
                    let contributors = evidence
                        .lineage
                        .get(output)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    match contributors {
                        [] => Some(UnattributedReason::NoResponse),
                        [one] if one.binding != mapping.binding => {
                            Some(UnattributedReason::WrongBinding)
                        }
                        [one] if one.actual_model.is_none() => {
                            Some(UnattributedReason::UnknownActualModel)
                        }
                        [one] => {
                            if one.question_kind != Some(mapping.question_kind) {
                                return Err(EvidenceError::InvalidRoster(format!(
                                    "Question kind mismatch for mapped output {output}"
                                )));
                            }
                            let key = (one.binding.clone(), one.actual_model.clone());
                            let backed = evidence.calls.iter().any(|call| {
                                call.completed
                                    && call.binding == one.binding
                                    && call.actual_model == one.actual_model
                            });
                            if !backed {
                                Some(UnattributedReason::NoResponse)
                            } else if let Some(entry) = entries.get_mut(&key) {
                                entry
                                    .cases
                                    .entry((output.clone(), case.label_provenance.kind))
                                    .or_default()
                                    .insert(case.id.clone());
                                None
                            } else {
                                Some(UnattributedReason::NoResponse)
                            }
                        }
                        _ => Some(UnattributedReason::MultipleResponses),
                    }
                }
            };
            if let Some(reason) = reason {
                unattributed.push(UnattributedCase {
                    case: case.id.clone(),
                    output: output.clone(),
                    reason,
                });
            }
        }
    }
    let mut profiles = Vec::new();
    for ((binding, actual_model), entry) in entries {
        let mut outputs = BTreeMap::<String, BTreeMap<LabelKind, RosterMetric>>::new();
        if actual_model.is_some() {
            for ((output, kind), ids) in entry.cases {
                let subset = Dataset {
                    id: dataset.id.clone(),
                    cases: dataset
                        .selected(split)
                        .filter(|case| ids.contains(&case.id))
                        .map(|case| {
                            let mut case = case.clone();
                            case.labels.retain(|name, _| name == &output);
                            case
                        })
                        .collect::<Vec<Case>>(),
                };
                let selected = subset
                    .cases
                    .iter()
                    .filter_map(|case| {
                        cases
                            .get(&case.id)
                            .map(|e| (case.id.clone(), e.outcome.clone()))
                    })
                    .collect();
                let schema = BTreeMap::from([(output.clone(), schemas[&output].clone())]);
                let measurement = measure(&subset, split, &schema, &selected, max_cases)?;
                if let Some(value) = measurement.outputs.get(&output) {
                    outputs.entry(output).or_default().insert(
                        kind,
                        RosterMetric {
                            case_ids: ids.into_iter().collect(),
                            measurement: value.clone(),
                            ece: None,
                            ece_absent_reason: Some("not_computed".into()),
                        },
                    );
                }
            }
        }
        profiles.push(RosterEntry {
            binding,
            actual_model,
            descriptor: entry.descriptor,
            calls: entry.calls,
            input_tokens: total(&entry.input)?,
            output_tokens: total(&entry.output)?,
            billing_units: total(&entry.billing)?,
            latency: latency(entry.durations)?,
            outputs,
        });
    }
    Ok(Roster {
        version: 1,
        dataset: dataset.id.clone(),
        split,
        graph_semantic_identity: graph_identity.into(),
        sapho_version: version.into(),
        mappings: mappings.clone(),
        entries: profiles,
        unattributed,
    })
}
