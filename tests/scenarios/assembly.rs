// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Record/list composition through public engine seams.
use crate::common::*;
use sapho::{core::*, graph::*};
use std::collections::BTreeMap;

/// Trace: FR-030-AC-1, FR-030-AC-2, FR-030-AC-3
#[tokio::test]
async fn assembled_records_project_fields_and_keep_sources() {
    let source = SourceRef {
        source: SourceId::new("local").unwrap(),
        start: Some(0),
        end: Some(2),
    };
    let mut data = datum("flag", Value::Boolean(true));
    data.sources.push(source.clone());
    let spec = GraphSpec {
        inputs: BTreeMap::from([
            ("flag".into(), ValueType::Boolean),
            ("label".into(), ValueType::Text),
        ]),
        nodes: vec![
            node(
                "record",
                Operation::Record {},
                [("present", input("flag")), ("label", input("label"))],
            ),
            node(
                "negate",
                Operation::Not,
                [(
                    "value",
                    Binding::Node {
                        node: NodeId::new("record").unwrap(),
                        port: "result".into(),
                        path: vec!["present".into()],
                    },
                )],
            ),
        ],
        outputs: BTreeMap::from([
            ("result".into(), output("negate", "result")),
            ("record".into(), output("record", "result")),
        ]),
        subgraphs: BTreeMap::new(),
    };
    let run = engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    )
    .run(
        &Inputs::from([
            ("flag".into(), data),
            (
                "label".into(),
                datum("label", Value::Text("original".into())),
            ),
        ]),
        limits(),
    )
    .await
    .unwrap();
    assert_eq!(run.outputs["result"].value, Value::Boolean(false));
    assert_eq!(run.outputs["result"].sources, vec![source]);
    assert_eq!(
        run.outputs["record"].value,
        Value::Record(BTreeMap::from([
            ("present".into(), Value::Boolean(true)),
            ("label".into(), Value::Text("original".into()))
        ]))
    );
    let mut incompatible = spec.clone();
    incompatible.nodes[1].inputs.insert(
        "value".into(),
        Binding::Node {
            node: NodeId::new("record").unwrap(),
            port: "result".into(),
            path: vec!["label".into()],
        },
    );
    assert_eq!(
        compile(&incompatible, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
    assert_eq!(
        operation(Operation::Record {}, vec![]).await,
        Value::Record(BTreeMap::new())
    );
    assert!(
        GraphSpec::parse("nodes: [{id: a, operation: {kind: record, extra: 1}}]\noutputs: {}")
            .is_err()
    );
}
/// Trace: FR-031-AC-1, FR-031-AC-2, FR-031-AC-3
#[tokio::test]
async fn list_occurrences_follow_order_share_limits_and_keep_each_source() {
    let source = SourceRef {
        source: SourceId::new("local").unwrap(),
        start: None,
        end: None,
    };
    let mut original = datum("original", Value::Number(2.0));
    original.sources.push(source.clone());
    let binding = Binding::Literal {
        value: original,
        value_type: ValueType::Number,
    };
    let make = |order: Vec<String>| {
        graph(
            vec![node(
                "list",
                Operation::List {
                    item_type: ValueType::Number,
                    order,
                },
                [("a", binding.clone()), ("b", binding.clone())],
            )],
            output("list", "result"),
        )
    };
    let spec = make(vec!["b".into(), "a".into()]);
    let compiled = engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    );
    let run = compiled.run(&Inputs::new(), limits()).await.unwrap();
    let Value::List(items) = &run.outputs["result"].value else {
        panic!("list expected")
    };
    assert_eq!(items.len(), 2);
    assert_ne!(items[0].id, items[1].id);
    assert!(items[0].id.as_str().contains("\"b\""));
    assert_eq!(items[0].sources, vec![source]);
    assert_eq!(
        compiled
            .run(&Inputs::new(), limits())
            .await
            .unwrap()
            .outputs,
        run.outputs
    );
    let mut limited = limits();
    limited.collection_items = 1;
    assert_eq!(
        compiled
            .run(&Inputs::new(), limited)
            .await
            .unwrap_err()
            .error
            .code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(
        operation(
            Operation::List {
                item_type: ValueType::Number,
                order: vec![]
            },
            vec![]
        )
        .await,
        Value::List(vec![])
    );
    for order in [vec!["a".into()], vec!["a".into(), "a".into()]] {
        assert_eq!(
            compile(&make(order), &PrimitiveRegistry::default())
                .err()
                .unwrap()
                .code,
            ErrorCode::Config
        );
    }
    let mixed = graph(
        vec![node(
            "list",
            Operation::List {
                item_type: ValueType::Number,
                order: vec!["a".into()],
            },
            [("a", lit(Value::Boolean(true), ValueType::Boolean))],
        )],
        output("list", "result"),
    );
    assert_eq!(
        compile(&mixed, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::TypeMismatch
    );
}

/// Trace: FR-031-AC-3
#[tokio::test]
async fn nested_list_assembly_shares_item_and_data_budgets() {
    let spec = graph(
        vec![
            node(
                "inner",
                Operation::List {
                    item_type: ValueType::Number,
                    order: vec!["a".into(), "b".into()],
                },
                [
                    ("a", lit(Value::Number(2.0), ValueType::Number)),
                    ("b", lit(Value::Number(3.0), ValueType::Number)),
                ],
            ),
            node(
                "outer",
                Operation::List {
                    item_type: ValueType::list(ValueType::Number),
                    order: vec!["a".into(), "b".into()],
                },
                [
                    ("a", output("inner", "result")),
                    ("b", output("inner", "result")),
                ],
            ),
        ],
        output("outer", "result"),
    );
    let runner = engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    );
    let output = runner.run(&Inputs::new(), limits()).await.unwrap();
    let Value::List(items) = &output.outputs["result"].value else {
        panic!("list expected")
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].value, items[1].value);
    assert_ne!(items[0].id, items[1].id);
    let mut limited = limits();
    limited.collection_items = 3;
    assert_eq!(
        runner
            .run(&Inputs::new(), limited)
            .await
            .unwrap_err()
            .error
            .code,
        ErrorCode::LimitExceeded
    );
    let mut limited = limits();
    limited.data_bytes = 32;
    assert_eq!(
        runner
            .run(&Inputs::new(), limited)
            .await
            .unwrap_err()
            .error
            .code,
        ErrorCode::LimitExceeded
    );
}
