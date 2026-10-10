// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Evaluate supplied outputs, rank development candidates and export supervision.
use sapho_core::{Datum, Inputs, Value, ValueType, decode_json};
use sapho_evidence::*;
use std::{collections::BTreeMap, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let dataset: Dataset = decode_json(
        include_bytes!("../../../examples/data/review-dataset.json"),
        1_048_576,
    )?;
    dataset.validate(100)?;
    let schemas = BTreeMap::from([("needs_review".into(), ValueType::Boolean)]);
    // An embedding host supplies real engine outcomes. Here the tutorial's
    // supplied probability inputs are evaluated by a transparent threshold.
    let outcomes = dataset
        .cases
        .iter()
        .map(|case| {
            let Some(Value::Probability(support)) = case.inputs.get("support").map(|d| &d.value)
            else {
                return Err("Expected probability input".into());
            };
            Ok((
                case.id.clone(),
                CaseOutcome::Completed {
                    outputs: Inputs::from([(
                        "needs_review".into(),
                        Datum::new(case.id.as_str(), Value::Boolean(support.get() < 0.8))?,
                    )]),
                    models: Vec::new(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Box<dyn Error>>>()?;
    let measurement = measure(&dataset, Split::Development, &schemas, &outcomes, 100)?;
    assert!(measurement.complete());
    assert_eq!(measurement.selected_cases, 2);
    let ranked = rank(
        &[Candidate {
            id: sapho_core::SourceId::new("threshold-0.8")?,
            measurement,
        }],
        "needs_review",
        Metric::Agreement,
        16,
    )?;
    assert_eq!(ranked.first().map(|c| c.score), Some(1.0));
    // A Probability output is scored with Brier against the same Boolean labels.
    let probability_schemas = BTreeMap::from([("needs_review".into(), ValueType::Probability)]);
    let probability_outcomes = dataset
        .cases
        .iter()
        .map(|case| {
            Ok((
                case.id.clone(),
                CaseOutcome::Completed {
                    outputs: Inputs::from([(
                        "needs_review".into(),
                        Datum::new(
                            case.id.as_str(),
                            Value::Probability(sapho_core::Probability::new(0.5)?),
                        )?,
                    )]),
                    models: Vec::new(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Box<dyn Error>>>()?;
    let probability_report = measure(
        &dataset,
        Split::Development,
        &probability_schemas,
        &probability_outcomes,
        100,
    )?;
    assert!(
        matches!(probability_report.outputs.get("needs_review").map(|o| &o.metrics), Some(Metrics::Probability { brier: Some(v), .. }) if (*v - 0.25).abs() < 1e-12)
    );
    assert_eq!(
        rank(
            &[Candidate {
                id: sapho_core::SourceId::new("constant-0.5")?,
                measurement: probability_report
            }],
            "needs_review",
            Metric::Brier,
            16
        )?
        .first()
        .map(|c| c.score),
        Some(0.25)
    );
    let held_out = measure(&dataset, Split::HeldOut, &schemas, &outcomes, 100)?;
    assert_eq!(held_out.selected_cases, 1);
    let missing = measure(
        &dataset,
        Split::Development,
        &schemas,
        &BTreeMap::new(),
        100,
    )?;
    assert!(!missing.complete());
    let unsupported = measure(
        &dataset,
        Split::Development,
        &BTreeMap::from([("needs_review".into(), ValueType::Degree)]),
        &outcomes,
        100,
    )?;
    assert!(!unsupported.complete());
    let export = export_training(&dataset, 100, 1_048_576)?;
    let text = String::from_utf8(export)?;
    assert_eq!(text.lines().count(), 2);
    assert!(!text.contains("reserved"));
    println!("agreement=1.0; Brier=0.25; held-out cases=1; exported development rows=2");
    Ok(())
}
