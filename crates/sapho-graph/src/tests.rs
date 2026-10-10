// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Compiler acceptance tests: no evaluation or inference occurs.
use super::*;
use sapho_core::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn lit(value: Value, t: ValueType) -> Binding {
    Binding::Literal {
        value: Datum::new("literal", value).unwrap(),
        value_type: t,
    }
}
fn output(node: &str) -> Binding {
    Binding::Node {
        node: NodeId::new(node).unwrap(),
        port: "result".into(),
        path: vec![],
    }
}
fn node(id: &str, operation: Operation, inputs: BTreeMap<String, Binding>) -> NodeSpec {
    NodeSpec {
        id: NodeId::new(id).unwrap(),
        operation,
        inputs,
        guard: None,
    }
}
fn graph(nodes: Vec<NodeSpec>, output_binding: Binding) -> GraphSpec {
    GraphSpec {
        inputs: BTreeMap::new(),
        nodes,
        outputs: BTreeMap::from([("result".into(), output_binding)]),
        subgraphs: BTreeMap::new(),
    }
}
/// Trace: FR-063-AC-2, FR-063-AC-5, IT-009-SC-04
#[test]
fn semantic_identity_normalizes_representation_and_keeps_ordered_nodes() {
    let yaml = GraphSpec::parse(include_str!("../../../examples/reference/facts.yaml")).unwrap();
    let json = serde_json::to_string_pretty(&yaml).unwrap();
    let parsed = GraphSpec::parse_with_format(&json, GraphFormat::Json).unwrap();
    assert_eq!(
        graph_semantic_identity(&yaml).unwrap(),
        graph_semantic_identity(&parsed).unwrap()
    );
    let mut numeric = graph(vec![], lit(Value::Number(1.0), ValueType::Number));
    let baseline = graph_semantic_identity(&numeric).unwrap();
    assert_eq!(
        baseline,
        "graph-v1:sha256:ab43fcf6e41e198d75b2a5ae6c70348415a4b773855d65f092b6cb65d9a59ac4"
    );
    let json = serde_json::to_string(&numeric).unwrap();
    let exponent =
        GraphSpec::parse_with_format(&json.replace("1.0", "1e0"), GraphFormat::Json).unwrap();
    assert_eq!(baseline, graph_semantic_identity(&exponent).unwrap());
    numeric
        .outputs
        .insert("result".into(), lit(Value::Number(2.0), ValueType::Number));
    assert_ne!(baseline, graph_semantic_identity(&numeric).unwrap());
    let first = node(
        "first",
        Operation::Not,
        BTreeMap::from([(
            "value".into(),
            lit(Value::Boolean(true), ValueType::Boolean),
        )]),
    );
    let second = node(
        "second",
        Operation::Not,
        BTreeMap::from([(
            "value".into(),
            lit(Value::Boolean(false), ValueType::Boolean),
        )]),
    );
    let a = graph(vec![first.clone(), second.clone()], output("second"));
    let b = graph(vec![second, first], output("second"));
    assert_ne!(
        graph_semantic_identity(&a).unwrap(),
        graph_semantic_identity(&b).unwrap()
    );
}
fn err(spec: &GraphSpec) -> ErrorCode {
    compile(spec, &PrimitiveRegistry::default())
        .err()
        .unwrap()
        .code
}
/// Trace: FR-007-AC-1, FR-007-AC-2, FR-007-AC-3, TC-007
#[test]
fn yaml_and_json_share_a_checked_declarative_program() {
    let text = r#"
inputs:
  fact:
    kind: record
    fields:
      present: {kind: boolean}
outputs:
  result: {kind: node, node: negate, port: result}
nodes:
  - id: negate
    operation: {kind: not}
    inputs:
      value: {kind: input, name: fact, path: [present]}
"#;
    let spec = GraphSpec::parse(text).unwrap();
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(
        GraphSpec::parse_with_format(&json, GraphFormat::Json).unwrap(),
        spec
    );
    let compiled = compile(&spec, &PrimitiveRegistry::default()).unwrap();
    assert_eq!(compiled.signature().outputs["result"], ValueType::Boolean);
    assert_eq!(
        GraphSpec::parse(&text.replace("kind: not", "kind: script"))
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
    assert_eq!(
        GraphSpec::parse("unknown: 1\noutputs: {}\n")
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
    assert_eq!(
        GraphSpec::parse(&"x".repeat(1_048_577)).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
}
/// Trace: FR-007-AC-1, FR-007-AC-2
#[test]
fn question_text_and_code_parameters_are_preserved_in_both_formats() {
    let yaml = r#"
nodes:
  - id: questions
    operation:
      kind: questions
      questions:
        - id: present
          question: {kind: boolean, instructions: "Keep # / ${TEXT} unchanged", yes: Present, no: Absent}
  - id: custom
    operation:
      kind: code
      primitive: local
      params: {limit: {kind: number, value: 2.0}}
outputs:
  questions: {kind: node, node: questions, port: result}
"#;
    let spec = GraphSpec::parse(yaml).unwrap();
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(
        GraphSpec::parse_with_format(&json, GraphFormat::Json).unwrap(),
        spec
    );
    let Operation::Questions { questions } = &spec.nodes[0].operation else {
        panic!("questions expected")
    };
    let Question::Boolean { instructions, .. } = &questions[0].question else {
        panic!("boolean expected")
    };
    assert_eq!(instructions, "Keep # / ${TEXT} unchanged");
    assert_eq!(
        compile(&spec, &PrimitiveRegistry::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::UnknownPrimitive
    );
}
/// Trace: FR-007-AC-2
#[test]
fn constrained_configuration_refuses_duplicate_keys_and_yaml_extensions() {
    for yaml in [
        "outputs: {}\noutputs: {}",
        "outputs: {result: {kind: input, name: a, name: b}}",
        "outputs: !custom {}",
        "outputs: {<<: {}}",
        "outputs: &a {}\ninputs: *a",
        "outputs: {}\n---\noutputs: {}",
    ] {
        assert!(GraphSpec::parse(yaml).is_err(), "{yaml}");
    }
    assert_eq!(
        GraphSpec::parse("outputs: {}\n...\n[invalid")
            .unwrap_err()
            .code,
        ErrorCode::Config
    );
    assert_eq!(
        GraphSpec::parse("outputs: {}\n---\noutputs: {}")
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    for yaml in ["outputs: &a {}", "outputs: &a {}\ninputs: *a"] {
        assert_eq!(
            GraphSpec::parse(yaml).unwrap_err().code,
            ErrorCode::LimitExceeded
        );
    }
    for yaml in [
        "outputs: !custom {}",
        "outputs: {<<: {}}",
        "outputs: {}\noutputs: {}",
    ] {
        assert_eq!(GraphSpec::parse(yaml).unwrap_err().code, ErrorCode::Config);
    }
    for json in [
        r#"{"outputs":{},"outputs":{}}"#,
        r#"{"outputs":{"result":{"kind":"input","name":"a","name":"b"}}}"#,
    ] {
        assert_eq!(
            GraphSpec::parse_with_format(json, GraphFormat::Json)
                .unwrap_err()
                .code,
            ErrorCode::Config
        );
    }
    assert_eq!(GraphSpec::parse("outputs: {}\nnodes: [{id: x, operation: {kind: not}, guard: {kind: literal, value_type: {kind: boolean}, value: {id: x, value: {kind: boolean, value: yes}}}}]").unwrap_err().code,ErrorCode::Config);
    let deeply_nested = format!("outputs: {}", "[".repeat(130) + "0" + &"]".repeat(130));
    assert_eq!(
        GraphSpec::parse(&deeply_nested).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
}
/// Trace: FR-008-AC-1, FR-008-AC-2
#[test]
fn forward_references_are_ordered_and_invalid_topology_is_refused() {
    let a = node(
        "a",
        Operation::Not,
        BTreeMap::from([(
            "value".into(),
            lit(Value::Boolean(false), ValueType::Boolean),
        )]),
    );
    let b = node(
        "b",
        Operation::Not,
        BTreeMap::from([("value".into(), output("a"))]),
    );
    let compiled = compile(
        &graph(vec![b.clone(), a.clone()], output("b")),
        &PrimitiveRegistry::default(),
    )
    .unwrap();
    assert_eq!(compiled.stages()[0][0].spec().id.as_str(), "a");
    assert_eq!(compiled.stages()[1][0].spec().id.as_str(), "b");
    assert_eq!(
        err(&graph(vec![a.clone(), a], output("a"))),
        ErrorCode::DuplicateId
    );
    let cycle = node(
        "a",
        Operation::Not,
        BTreeMap::from([("value".into(), output("b"))]),
    );
    assert_eq!(err(&graph(vec![cycle, b], output("a"))), ErrorCode::Cycle);
    let unknown = node(
        "a",
        Operation::Not,
        BTreeMap::from([("value".into(), output("missing"))]),
    );
    assert_eq!(
        err(&graph(vec![unknown], output("a"))),
        ErrorCode::UnknownReference
    );
    let bad = node(
        "a",
        Operation::Not,
        BTreeMap::from([("value".into(), lit(Value::Number(1.0), ValueType::Number))]),
    );
    assert_eq!(err(&graph(vec![bad], output("a"))), ErrorCode::TypeMismatch);
}
struct CountNative(Arc<AtomicUsize>);
impl Primitive for CountNative {
    fn signature(&self) -> Signature {
        Signature {
            inputs: BTreeMap::new(),
            outputs: BTreeMap::from([("result".into(), ValueType::Boolean)]),
        }
    }
    fn execute(
        &self,
        _: &PrimitiveContext,
        _: &Inputs,
        _: &BTreeMap<String, Value>,
    ) -> Result<Inputs> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(SaphoError::new(
            ErrorCode::CodeFailed,
            "must not execute during compile",
        ))
    }
}
/// Trace: FR-008-AC-3, FR-003-AC-1
#[test]
fn compilation_binds_native_code_without_evaluating_it() {
    let counter = Arc::new(AtomicUsize::new(0));
    let id = PrimitiveId::new("consumer.fact").unwrap();
    let mut registry = PrimitiveRegistry::default();
    registry
        .register(id.clone(), Arc::new(CountNative(counter.clone())))
        .unwrap();
    let spec = graph(
        vec![node(
            "a",
            Operation::Code {
                primitive: id,
                params: BTreeMap::new(),
            },
            BTreeMap::new(),
        )],
        output("a"),
    );
    let compiled = compile(&spec, &registry).unwrap();
    assert!(compiled.stages()[0][0].primitive().is_some());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    let mut over = spec;
    over.nodes = vec![over.nodes[0].clone(); 4097];
    assert_eq!(
        compile(&over, &registry).err().unwrap().code,
        ErrorCode::LimitExceeded
    );
}
/// Trace: FR-009-AC-1, FR-018-AC-2, FR-019-AC-2, FR-023-AC-2, FR-024-AC-3
#[test]
fn guarded_outputs_and_numeric_semantics_require_explicit_connections() {
    let mut a = node(
        "a",
        Operation::Not,
        BTreeMap::from([(
            "value".into(),
            lit(Value::Boolean(false), ValueType::Boolean),
        )]),
    );
    a.guard = Some(lit(Value::Boolean(true), ValueType::Boolean));
    let b = node(
        "b",
        Operation::Not,
        BTreeMap::from([("value".into(), output("a"))]),
    );
    assert_eq!(
        err(&graph(vec![a.clone(), b], output("b"))),
        ErrorCode::TypeMismatch
    );
    let compiled = compile(&graph(vec![a], output("a")), &PrimitiveRegistry::default()).unwrap();
    assert_eq!(
        compiled.signature().outputs["result"],
        ValueType::optional(ValueType::Boolean)
    );
    let p = Value::Probability(Probability::new(0.7).unwrap());
    let n = node(
        "x",
        Operation::Complement,
        BTreeMap::from([("value".into(), lit(p, ValueType::Probability))]),
    );
    assert_eq!(err(&graph(vec![n], output("x"))), ErrorCode::TypeMismatch);
    let n = node(
        "x",
        Operation::Coalesce,
        BTreeMap::from([
            (
                "value".into(),
                lit(
                    Value::Optional(None),
                    ValueType::optional(ValueType::Number),
                ),
            ),
            (
                "default".into(),
                lit(Value::Text("wrong".into()), ValueType::Text),
            ),
        ]),
    );
    assert_eq!(err(&graph(vec![n], output("x"))), ErrorCode::TypeMismatch);
}
/// Trace: FR-008-AC-2, FR-008-AC-3
#[test]
fn even_unused_subgraphs_are_validated_for_recursion() {
    let body = GraphBody {
        inputs: BTreeMap::from([("item".into(), ValueType::Boolean)]),
        nodes: vec![node(
            "map",
            Operation::Map {
                graph: "recursive".into(),
            },
            BTreeMap::from([(
                "items".into(),
                lit(Value::List(vec![]), ValueType::list(ValueType::Boolean)),
            )]),
        )],
        outputs: BTreeMap::from([("result".into(), output("map"))]),
    };
    let mut spec = graph(vec![], lit(Value::Boolean(true), ValueType::Boolean));
    spec.subgraphs.insert("recursive".into(), body);
    assert_eq!(err(&spec), ErrorCode::Cycle);
}

/// Trace: FR-081-AC-2, FR-082-AC-2, FR-082-AC-4, IT-017-SC-04
#[test]
fn calibrated_ports_require_explicit_conversion_and_valid_map_literal() {
    let base =
        GraphSpec::parse(include_str!("../../../examples/reference/calibration.yaml")).unwrap();
    let compiled = compile(&base, &PrimitiveRegistry::default()).unwrap();
    assert_eq!(
        compiled.signature().outputs["fitted"],
        ValueType::CalibratedProbability
    );
    assert_eq!(
        compiled.signature().outputs["converted"],
        ValueType::Probability
    );
    let mut wrong_map = base.clone();
    wrong_map.nodes[0].inputs.insert(
        "map".into(),
        lit(
            Value::Probability(Probability::new(0.5).unwrap()),
            ValueType::Probability,
        ),
    );
    assert_eq!(err(&wrong_map), ErrorCode::TypeMismatch);
    let mut implicit = base.clone();
    implicit.nodes[1].inputs.insert(
        "value".into(),
        lit(
            Value::Probability(Probability::new(0.5).unwrap()),
            ValueType::Probability,
        ),
    );
    assert_eq!(err(&implicit), ErrorCode::TypeMismatch);
    let mut implicit = base.clone();
    implicit.nodes.push(node(
        "degree",
        Operation::Degree,
        BTreeMap::from([("value".into(), output("calibrate"))]),
    ));
    implicit.outputs.insert("degree".into(), output("degree"));
    assert_eq!(err(&implicit), ErrorCode::TypeMismatch);
    let mut malformed = base;
    let Binding::Literal { value, .. } = malformed.nodes[0].inputs.get_mut("map").unwrap() else {
        unreachable!()
    };
    let Value::CalibrationMap(map) = &mut value.value else {
        unreachable!()
    };
    map.map_id = "calibration-v1:sha256:deadbeef".into();
    assert_eq!(err(&malformed), ErrorCode::InvalidValue);
}
