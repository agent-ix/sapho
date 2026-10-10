// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Acquire files, Git patches and JSON before entering an async evaluator.
use sapho_core::{Value, ValueType, decode_json};
use sapho_select::*;
use std::{error::Error, path::Path};

fn count(inputs: &sapho_core::Inputs, port: &str) -> usize {
    match inputs.get(port).map(|d| &d.value) {
        Some(Value::List(items)) => items.len(),
        _ => 0,
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let limits = SelectionLimits::default();
    let patterns = Patterns {
        include: vec!["**/*.yaml".into()],
        exclude: vec![
            "**/review-conservative.yaml".into(),
            "**/router.yaml".into(),
            "**/escalation.yaml".into(),
        ],
    };
    let files = select_files(&root.join("examples/graphs"), &patterns, &limits, "items")?;
    assert_eq!(count(&files, "items"), 7);
    let schema: ValueType = decode_json(
        include_bytes!("../../../examples/data/selection-schema.json"),
        1_048_576,
    )?;
    let json = select_json(
        include_bytes!("../../../examples/data/selection.json"),
        "/records",
        &schema,
        "requirements",
        "items",
        "examples/data/selection.json",
        1_048_576,
    )?;
    assert_eq!(count(&json, "items"), 2);
    assert!(json.get("items").is_some_and(|d| !d.sources.is_empty()));
    let refused = select_json(
        b"{}",
        "/missing",
        &schema,
        "requirements",
        "items",
        "inline",
        1024,
    );
    assert!(matches!(refused, Err(SelectionError::MissingKey(_))));
    // This optional argument reads the user's repository without changing it.
    // Git 2.34+ and a Unix host are required. No Git process is used by default.
    if std::env::args().any(|arg| arg == "--git") {
        for mode in [
            GitMode::WorkingTree,
            GitMode::Staged,
            GitMode::Revisions {
                base: "HEAD".into(),
                head: "HEAD".into(),
            },
        ] {
            let patches = select_git(
                &root,
                &GitOptions {
                    mode,
                    ..GitOptions::default()
                },
                &Patterns::default(),
                &limits,
                "items",
            )?;
            println!("Git patches: {}", count(&patches, "items"));
        }
    }
    println!("selected YAML files=7; JSON records=2; missing member refused");
    Ok(())
}
