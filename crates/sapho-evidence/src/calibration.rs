// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Deterministic development calibration over checked host outcomes.
use crate::{CaseOutcome, Dataset, EvidenceError, LabelKind, Split};
use sapho_core::{
    BackendId, CalibratedProbability, CalibrationKnot, CalibrationMap, ItemId, ModelIdentity,
    Probability, Value, canonical_sha256,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// One completed response's checked contributing binding and reported model.
pub type FitAttribution = BTreeMap<ItemId, (BackendId, ModelIdentity)>;

fn invalid(reason: &'static str) -> EvidenceError {
    EvidenceError::InvalidCalibration(reason.into())
}

#[derive(Serialize)]
struct CaseDigestRow<'a> {
    case_id: &'a ItemId,
    inputs: &'a sapho_core::Inputs,
    label: bool,
    provenance: &'a crate::LabelProvenance,
}
#[derive(Serialize)]
struct ObservationDigestRow<'a> {
    case_id: &'a ItemId,
    raw: Probability,
    label: bool,
    binding: &'a BackendId,
    actual_model: &'a str,
}
struct Row<'a> {
    case: &'a crate::Case,
    raw: Probability,
    label: bool,
    binding: &'a BackendId,
    model: &'a str,
}
struct Block {
    first: usize,
    last: usize,
    positive: u64,
    total: u64,
}
/// Digest selected development case content without reading held-out labels or predictions.
pub fn calibration_cases_digest(
    dataset: &Dataset,
    output: &str,
    max_cases: usize,
) -> Result<String, EvidenceError> {
    dataset.validate(max_cases)?;
    let mut cases = dataset
        .selected(Split::Development)
        .filter_map(|case| case.labels.get(output).map(|label| (case, *label)))
        .collect::<Vec<_>>();
    cases.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    let hash = canonical_sha256(
        "sapho.dataset.development.v1\n",
        &(
            &dataset.id,
            output,
            cases
                .iter()
                .map(|(case, label)| CaseDigestRow {
                    case_id: &case.id,
                    inputs: &case.inputs,
                    label: *label,
                    provenance: &case.label_provenance,
                })
                .collect::<Vec<_>>(),
        ),
    )?;
    Ok(format!("dataset-development-v1:sha256:{hash}"))
}

/// Fit a versioned monotone map from one fully scored development output.
/// The host supplies trace attribution without importing runtime into this crate.
pub fn fit_calibration(
    dataset: &Dataset,
    split: Split,
    output: &str,
    outcomes: &BTreeMap<ItemId, CaseOutcome>,
    attribution: &FitAttribution,
    graph_identity: &str,
    max_cases: usize,
) -> Result<CalibrationMap, EvidenceError> {
    if split != Split::Development {
        return Err(invalid("held_out_fit"));
    }
    dataset.validate(max_cases)?;
    if output.trim().is_empty() {
        return Err(invalid("output_absent"));
    }
    let mut rows = Vec::new();
    for case in dataset.selected(split) {
        let Some(&label) = case.labels.get(output) else {
            continue;
        };
        let (binding, identity) = attribution
            .get(&case.id)
            .ok_or_else(|| invalid("attribution_absent"))?;
        if identity.name.trim().is_empty() {
            return Err(invalid("actual_model_absent"));
        }
        let raw = match outcomes.get(&case.id) {
            Some(CaseOutcome::Completed { outputs, models }) => {
                if case.label_provenance.kind == LabelKind::Model
                    && models
                        .iter()
                        .any(|model| model.name == case.label_provenance.source)
                {
                    return Err(invalid("self_source"));
                }
                match outputs.get(output).map(|datum| &datum.value) {
                    Some(Value::Probability(raw)) => *raw,
                    _ => return Err(invalid("raw_probability_absent")),
                }
            }
            _ => return Err(invalid("case_failed")),
        };
        rows.push(Row {
            case,
            raw,
            label,
            binding,
            model: &identity.name,
        });
    }
    if rows.len() < 20 {
        return Err(invalid("too_few_cases"));
    }
    let positives = rows.iter().filter(|row| row.label).count();
    if positives < 2 || rows.len() - positives < 2 {
        return Err(invalid("class_count"));
    }
    let binding = rows[0].binding;
    let model = rows[0].model;
    if rows
        .iter()
        .any(|row| row.binding != binding || row.model != model)
    {
        return Err(invalid("mixed_model"));
    }
    rows.sort_by(|a, b| {
        a.raw
            .get()
            .total_cmp(&b.raw.get())
            .then_with(|| a.case.id.cmp(&b.case.id))
    });
    let mut groups: Vec<(Probability, u64, u64)> = Vec::new();
    for row in &rows {
        if let Some((_, positive, total)) = groups
            .last_mut()
            .filter(|(raw, _, _)| raw.get() == row.raw.get())
        {
            *positive += u64::from(row.label);
            *total += 1;
        } else {
            groups.push((row.raw, u64::from(row.label), 1));
        }
    }
    if groups.len() < 2 || groups.len() > 4096 {
        return Err(invalid("raw_group_count"));
    }
    let mut blocks: Vec<Block> = Vec::new();
    for (index, (_, positive, total)) in groups.iter().copied().enumerate() {
        blocks.push(Block {
            first: index,
            last: index,
            positive,
            total,
        });
        while blocks.len() >= 2 {
            let right = blocks.last().expect("two blocks");
            let left = &blocks[blocks.len() - 2];
            let descending = left
                .positive
                .checked_mul(right.total)
                .ok_or_else(|| invalid("rate_overflow"))?
                > right
                    .positive
                    .checked_mul(left.total)
                    .ok_or_else(|| invalid("rate_overflow"))?;
            if !descending {
                break;
            }
            let right = blocks.pop().expect("two blocks");
            let left = blocks.last_mut().expect("left block");
            left.last = right.last;
            left.positive = left
                .positive
                .checked_add(right.positive)
                .ok_or_else(|| invalid("count_overflow"))?;
            left.total = left
                .total
                .checked_add(right.total)
                .ok_or_else(|| invalid("count_overflow"))?;
        }
    }
    let mut knots = Vec::with_capacity(groups.len());
    for block in blocks {
        let value = CalibratedProbability::new(block.positive as f64 / block.total as f64)?;
        for (raw, _, _) in &groups[block.first..=block.last] {
            knots.push(CalibrationKnot {
                raw: *raw,
                calibrated: value,
            });
        }
    }
    let mut cases = rows.iter().collect::<Vec<_>>();
    cases.sort_by(|a, b| a.case.id.cmp(&b.case.id));
    let case_digest = calibration_cases_digest(dataset, output, max_cases)?;
    let observation_digest = canonical_sha256(
        "sapho.calibration.observations.v1\n",
        &cases
            .iter()
            .map(|row| ObservationDigestRow {
                case_id: &row.case.id,
                raw: row.raw,
                label: row.label,
                binding: row.binding,
                actual_model: row.model,
            })
            .collect::<Vec<_>>(),
    )?;
    Ok(CalibrationMap::new(
        dataset.id.clone(),
        case_digest,
        format!("calibration-observations-v1:sha256:{observation_digest}"),
        graph_identity.into(),
        output.into(),
        binding.clone(),
        model.into(),
        u32::try_from(rows.len()).map_err(|_| invalid("case_count"))?,
        knots,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Case, LabelKind, LabelProvenance, Metrics, measure};
    use sapho_core::{Datum, ModelIdentity, SourceId, ValueType};
    fn fixture() -> (Dataset, BTreeMap<ItemId, CaseOutcome>, FitAttribution) {
        let mut cases = Vec::new();
        let mut outcomes = BTreeMap::new();
        let mut attribution = BTreeMap::new();
        for index in 0..20 {
            let id = ItemId::new(format!("case-{index:02}")).unwrap();
            let raw = if index < 10 { 0.05 } else { 0.95 };
            let label = if index < 10 { index < 2 } else { index < 18 };
            let inputs = BTreeMap::from([(
                "passage".into(),
                Datum::new(
                    format!("input-{index}"),
                    Value::Text(format!("text {index}")),
                )
                .unwrap(),
            )]);
            cases.push(Case {
                id: id.clone(),
                split: Split::Development,
                inputs,
                labels: BTreeMap::from([
                    ("raw_support".into(), label),
                    ("calibrated_support".into(), label),
                ]),
                label_provenance: LabelProvenance {
                    kind: LabelKind::Human,
                    source: "annotator".into(),
                    reference: format!("row-{index}"),
                },
            });
            outcomes.insert(
                id.clone(),
                CaseOutcome::Completed {
                    outputs: BTreeMap::from([(
                        "raw_support".into(),
                        Datum::new(
                            format!("raw-{index}"),
                            Value::Probability(Probability::new(raw).unwrap()),
                        )
                        .unwrap(),
                    )]),
                    models: vec![ModelIdentity {
                        name: "model-a".into(),
                    }],
                },
            );
            attribution.insert(
                id,
                (
                    BackendId::new("fast_a").unwrap(),
                    ModelIdentity {
                        name: "model-a".into(),
                    },
                ),
            );
        }
        (
            Dataset {
                id: SourceId::new("curated").unwrap(),
                cases,
            },
            outcomes,
            attribution,
        )
    }
    /// Trace: FR-083-AC-1, FR-083-AC-3, FR-083-AC-4, FR-083-AC-6
    #[test]
    fn twenty_case_fit_is_deterministic_and_scores_separate_quantities() {
        let (mut data, mut outcomes, attribution) = fixture();
        let graph = format!("graph-v1:sha256:{}", "a".repeat(64));
        let map = fit_calibration(
            &data,
            Split::Development,
            "raw_support",
            &outcomes,
            &attribution,
            &graph,
            100,
        )
        .unwrap();
        assert_eq!(map.case_count, 20);
        assert_eq!(
            map.knots
                .iter()
                .map(|k| (k.raw.get(), k.calibrated.get()))
                .collect::<Vec<_>>(),
            vec![(0.05, 0.2), (0.95, 0.8)]
        );
        let original = serde_json::to_vec(&map).unwrap();
        data.cases.reverse();
        let again = fit_calibration(
            &data,
            Split::Development,
            "raw_support",
            &outcomes,
            &attribution,
            &graph,
            100,
        )
        .unwrap();
        assert_eq!(original, serde_json::to_vec(&again).unwrap());
        assert!(
            fit_calibration(
                &data,
                Split::HeldOut,
                "raw_support",
                &outcomes,
                &attribution,
                &graph,
                100
            )
            .is_err()
        );
        let mut model_b = attribution.clone();
        model_b.get_mut(&data.cases[0].id).unwrap().1.name = "model-b".into();
        assert!(
            fit_calibration(
                &data,
                Split::Development,
                "raw_support",
                &outcomes,
                &model_b,
                &graph,
                100
            )
            .is_err()
        );
        for outcome in outcomes.values_mut() {
            if let CaseOutcome::Completed { outputs, .. } = outcome {
                let raw = match outputs["raw_support"].value {
                    Value::Probability(p) => p,
                    _ => unreachable!(),
                };
                outputs.insert(
                    "calibrated_support".into(),
                    Datum::new(
                        "calibrated",
                        Value::CalibratedProbability(map.apply(raw).unwrap()),
                    )
                    .unwrap(),
                );
            }
        }
        let report = measure(
            &data,
            Split::Development,
            &BTreeMap::from([
                ("raw_support".into(), ValueType::Probability),
                (
                    "calibrated_support".into(),
                    ValueType::CalibratedProbability,
                ),
            ]),
            &outcomes,
            100,
        )
        .unwrap();
        let Metrics::Probability {
            brier: Some(raw_brier),
            ece: Some(raw_ece),
            ..
        } = report.outputs["raw_support"].metrics
        else {
            panic!("raw metrics")
        };
        let Metrics::CalibratedProbability {
            brier: Some(fitted_brier),
            ece: Some(fitted_ece),
            ..
        } = report.outputs["calibrated_support"].metrics
        else {
            panic!("fitted metrics")
        };
        assert!((raw_brier - 0.1825).abs() < 1e-9);
        assert!((raw_ece - 0.15).abs() < 1e-9);
        assert!((fitted_brier - 0.16).abs() < 1e-9);
        assert!(fitted_ece.abs() < 1e-9);
    }
    /// Trace: FR-083-AC-2, FR-083-AC-3, FR-083-AC-6
    #[test]
    fn pava_pools_descending_group_and_refuses_ineligible_rows() {
        let (mut data, mut outcomes, attribution) = fixture();
        let graph = format!("graph-v1:sha256:{}", "a".repeat(64));
        for case in &data.cases[10..15] {
            if let CaseOutcome::Completed { outputs, .. } = outcomes.get_mut(&case.id).unwrap() {
                outputs.get_mut("raw_support").unwrap().value =
                    Value::Probability(Probability::new(0.5).unwrap());
            }
        }
        let map = fit_calibration(
            &data,
            Split::Development,
            "raw_support",
            &outcomes,
            &attribution,
            &graph,
            100,
        )
        .unwrap();
        assert_eq!(
            map.knots
                .iter()
                .map(|k| (k.raw.get(), k.calibrated.get()))
                .collect::<Vec<_>>(),
            vec![(0.05, 0.2), (0.5, 0.8), (0.95, 0.8)]
        );
        let old_digest = map.fit_observation_digest.clone();
        if let CaseOutcome::Completed { outputs, .. } = outcomes.get_mut(&data.cases[0].id).unwrap()
        {
            outputs.get_mut("raw_support").unwrap().value =
                Value::Probability(Probability::new(0.06).unwrap());
        }
        let changed = fit_calibration(
            &data,
            Split::Development,
            "raw_support",
            &outcomes,
            &attribution,
            &graph,
            100,
        )
        .unwrap();
        assert_ne!(old_digest, changed.fit_observation_digest);
        let mut model_b = attribution.clone();
        for value in model_b.values_mut() {
            value.1.name = "model-b".into();
        }
        let renamed = fit_calibration(
            &data,
            Split::Development,
            "raw_support",
            &outcomes,
            &model_b,
            &graph,
            100,
        )
        .unwrap();
        assert_ne!(changed.map_id, renamed.map_id);
        data.cases.pop();
        assert!(
            fit_calibration(
                &data,
                Split::Development,
                "raw_support",
                &outcomes,
                &attribution,
                &graph,
                100
            )
            .is_err()
        );
        let (data, mut outcomes, attribution) = fixture();
        if let CaseOutcome::Completed { outputs, .. } = outcomes.get_mut(&data.cases[0].id).unwrap()
        {
            outputs.remove("raw_support");
        }
        assert!(
            fit_calibration(
                &data,
                Split::Development,
                "raw_support",
                &outcomes,
                &attribution,
                &graph,
                100
            )
            .is_err()
        );
    }
}
