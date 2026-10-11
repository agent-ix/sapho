// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synthetic consumer composition; no domain implementation is exported by Sapho.
use crate::common::*;
use sapho::{core::*, graph::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
const STATEMENT: &str = "When the client uploads a file, the service shall store it; when the service sends a notice, the client shall display it.";
fn text_field(value: &Value, key: &str) -> String {
    let Value::Record(fields) = value else {
        panic!("record expected")
    };
    let Value::Text(text) = &fields[key] else {
        panic!("text expected")
    };
    text.clone()
}
fn phrase_type() -> ValueType {
    record_type([("id", ValueType::Text), ("surface", ValueType::Text)])
}
fn role_type() -> ValueType {
    record_type([
        ("id", ValueType::Text),
        ("surface", ValueType::Text),
        ("role", ValueType::Text),
    ])
}
fn pair_type() -> ValueType {
    record_type([("left", role_type()), ("right", role_type())])
}
fn relation_type() -> ValueType {
    record_type([
        ("actor", ValueType::Text),
        ("action", ValueType::Text),
        ("selected", ValueType::Boolean),
    ])
}
fn native(
    registry: &mut PrimitiveRegistry,
    id: &str,
    inputs: impl IntoIterator<Item = (&'static str, ValueType)>,
    ty: ValueType,
    f: impl Fn(&Inputs, &BTreeMap<String, Value>) -> Result<Inputs> + Send + Sync + 'static,
) {
    registry
        .register(
            PrimitiveId::new(id).unwrap(),
            Arc::new(Native {
                signature: Signature {
                    inputs: inputs.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                    outputs: BTreeMap::from([("result".into(), ty)]),
                },
                f: Box::new(f),
            }),
        )
        .unwrap();
}
fn code(id: &str) -> Operation {
    Operation::Code {
        primitive: PrimitiveId::new(id).unwrap(),
        params: BTreeMap::new(),
    }
}
fn result(value: Value) -> Result<Inputs> {
    Ok(BTreeMap::from([(
        "result".into(),
        datum("native-result", value),
    )]))
}
fn definitions() -> (GraphSpec, PrimitiveRegistry) {
    let mut registry = PrimitiveRegistry::default();
    native(
        &mut registry,
        "extract",
        [("statement", ValueType::Text)],
        ValueType::list(phrase_type()),
        |input, _| {
            let Value::Text(statement) = &input["statement"].value else {
                panic!("type checked")
            };
            let mut phrases = Vec::new();
            for word in ["client", "service", "uploads", "store", "sends", "display"] {
                for (offset, _) in statement.match_indices(word) {
                    let id = format!("word-{offset}");
                    let mut item = datum(
                        &id,
                        record([
                            ("id", Value::Text(id.clone())),
                            ("surface", Value::Text(word.into())),
                        ]),
                    );
                    item.sources.push(SourceRef {
                        source: SourceId::new("synthetic-statement").unwrap(),
                        start: Some(u64::try_from(offset).unwrap()),
                        end: Some(u64::try_from(offset + word.len()).unwrap()),
                    });
                    phrases.push((offset, item));
                }
            }
            phrases.sort_by_key(|(offset, _)| *offset);
            result(Value::List(
                phrases.into_iter().map(|(_, item)| item).collect(),
            ))
        },
    );
    native(
        &mut registry,
        "role_state",
        [("item", phrase_type()), ("statement", ValueType::Text)],
        record_type([("surface", ValueType::Text), ("context", ValueType::Text)]),
        |i, _| {
            result(record([
                (
                    "surface",
                    Value::Text(text_field(&i["item"].value, "surface")),
                ),
                ("context", i["statement"].value.clone()),
            ]))
        },
    );
    native(
        &mut registry,
        "pack_role",
        [("item", phrase_type()), ("answers", ValueType::Answers)],
        role_type(),
        |i, _| {
            let Value::Answers(a) = &i["answers"].value else {
                panic!("answers expected")
            };
            let Answer::Choice { selected, .. } = &a.values["role"] else {
                panic!("choice expected")
            };
            result(record([
                ("id", Value::Text(text_field(&i["item"].value, "id"))),
                (
                    "surface",
                    Value::Text(text_field(&i["item"].value, "surface")),
                ),
                ("role", Value::Text(selected.clone())),
            ]))
        },
    );
    native(
        &mut registry,
        "is_role",
        [("item", role_type()), ("want", ValueType::Text)],
        ValueType::Boolean,
        |i, _| {
            result(Value::Boolean(
                Value::Text(text_field(&i["item"].value, "role")) == i["want"].value,
            ))
        },
    );
    native(
        &mut registry,
        "relation_state",
        [("item", pair_type()), ("statement", ValueType::Text)],
        record_type([
            ("actor", ValueType::Text),
            ("action", ValueType::Text),
            ("context", ValueType::Text),
        ]),
        |i, _| {
            let Value::Record(pair) = &i["item"].value else {
                panic!("pair expected")
            };
            result(record([
                ("actor", Value::Text(text_field(&pair["left"], "id"))),
                ("action", Value::Text(text_field(&pair["right"], "id"))),
                ("context", i["statement"].value.clone()),
            ]))
        },
    );
    native(
        &mut registry,
        "relation_questions",
        [("item", pair_type())],
        ValueType::Questions,
        |i, _| {
            let Value::Record(pair) = &i["item"].value else {
                panic!("pair expected")
            };
            result(Value::Questions(vec![NamedQuestion {
                id: "related".into(),
                question: Question::Boolean {
                    instructions: format!(
                        "Does this {} perform this {} in the statement?",
                        text_field(&pair["left"], "role"),
                        text_field(&pair["right"], "role")
                    ),
                    yes: "The stated actor performs the stated action".into(),
                    no: "There is no such relationship".into(),
                },
            }]))
        },
    );
    native(
        &mut registry,
        "pack_relation",
        [("item", pair_type()), ("selected", ValueType::Boolean)],
        relation_type(),
        |i, _| {
            let Value::Record(pair) = &i["item"].value else {
                panic!("pair expected")
            };
            result(record([
                ("actor", Value::Text(text_field(&pair["left"], "id"))),
                ("action", Value::Text(text_field(&pair["right"], "id"))),
                ("selected", i["selected"].value.clone()),
            ]))
        },
    );
    native(
        &mut registry,
        "assemble",
        [("relations", ValueType::list(relation_type()))],
        ValueType::list(relation_type()),
        |i, _| {
            let Value::List(relations) = &i["relations"].value else {
                panic!("list expected")
            };
            result(Value::List(
                relations
                    .iter()
                    .filter(|item| {
                        let Value::Record(r) = &item.value else {
                            panic!("record expected")
                        };
                        r["selected"] == Value::Boolean(true)
                    })
                    .cloned()
                    .collect(),
            ))
        },
    );
    let roles = GraphBody {
        inputs: BTreeMap::from([
            ("item".into(), phrase_type()),
            ("statement".into(), ValueType::Text),
        ]),
        nodes: vec![
            node(
                "state",
                code("role_state"),
                [("item", input("item")), ("statement", input("statement"))],
            ),
            node(
                "questions",
                Operation::Questions {
                    questions: vec![NamedQuestion {
                        id: "role".into(),
                        question: Question::Choice {
                            instructions: "Classify the candidate phrase in its complete statement"
                                .into(),
                            options: ["actor", "action", "other"]
                                .into_iter()
                                .map(|label| ChoiceOption {
                                    label: label.into(),
                                    description: label.into(),
                                })
                                .collect(),
                        },
                    }],
                },
                [],
            ),
            node(
                "ask",
                Operation::Ask {
                    samples: 1,
                    backend: BackendId::new("judge").unwrap(),
                },
                [
                    ("state", output("state", "result")),
                    ("questions", output("questions", "result")),
                ],
            ),
            node(
                "pack",
                code("pack_role"),
                [
                    ("item", input("item")),
                    ("answers", output("ask", "answers")),
                ],
            ),
        ],
        outputs: BTreeMap::from([("result".into(), output("pack", "result"))]),
    };
    let masks = GraphBody {
        inputs: BTreeMap::from([
            ("item".into(), role_type()),
            ("want".into(), ValueType::Text),
        ]),
        nodes: vec![node(
            "mask",
            code("is_role"),
            [("item", input("item")), ("want", input("want"))],
        )],
        outputs: BTreeMap::from([("result".into(), output("mask", "result"))]),
    };
    let relations = GraphBody {
        inputs: BTreeMap::from([
            ("item".into(), pair_type()),
            ("statement".into(), ValueType::Text),
        ]),
        nodes: vec![
            node(
                "state",
                code("relation_state"),
                [("item", input("item")), ("statement", input("statement"))],
            ),
            node(
                "questions",
                code("relation_questions"),
                [("item", input("item"))],
            ),
            node(
                "ask",
                Operation::Ask {
                    samples: 1,
                    backend: BackendId::new("judge").unwrap(),
                },
                [
                    ("state", output("state", "result")),
                    ("questions", output("questions", "result")),
                ],
            ),
            node(
                "probability",
                Operation::Probability {
                    question: "related".into(),
                    labels: vec!["true".into()],
                },
                [("answers", output("ask", "answers"))],
            ),
            node(
                "degree",
                Operation::Degree,
                [("value", output("probability", "result"))],
            ),
            node(
                "threshold",
                Operation::Compare {
                    comparator: Comparator::GreaterEqual,
                },
                [
                    ("a", output("degree", "result")),
                    ("b", lit(degree(0.5), ValueType::Degree)),
                ],
            ),
            node(
                "pack",
                code("pack_relation"),
                [
                    ("item", input("item")),
                    ("selected", output("threshold", "result")),
                ],
            ),
        ],
        outputs: BTreeMap::from([("result".into(), output("pack", "result"))]),
    };
    let mut spec = graph(
        vec![
            node(
                "extract",
                code("extract"),
                [("statement", input("statement"))],
            ),
            node(
                "roles",
                Operation::Map {
                    graph: "roles".into(),
                },
                [
                    ("items", output("extract", "result")),
                    ("statement", input("statement")),
                ],
            ),
            node(
                "actor_masks",
                Operation::Map {
                    graph: "masks".into(),
                },
                [
                    ("items", output("roles", "result")),
                    ("want", lit(Value::Text("actor".into()), ValueType::Text)),
                ],
            ),
            node(
                "action_masks",
                Operation::Map {
                    graph: "masks".into(),
                },
                [
                    ("items", output("roles", "result")),
                    ("want", lit(Value::Text("action".into()), ValueType::Text)),
                ],
            ),
            node(
                "actors",
                Operation::Filter,
                [
                    ("items", output("roles", "result")),
                    ("mask", output("actor_masks", "result")),
                ],
            ),
            node(
                "actions",
                Operation::Filter,
                [
                    ("items", output("roles", "result")),
                    ("mask", output("action_masks", "result")),
                ],
            ),
            node(
                "pairs",
                Operation::Pairs,
                [
                    ("left", output("actors", "result")),
                    ("right", output("actions", "result")),
                ],
            ),
            node(
                "relations",
                Operation::Map {
                    graph: "relations".into(),
                },
                [
                    ("items", output("pairs", "result")),
                    ("statement", input("statement")),
                ],
            ),
            node(
                "assembled",
                code("assemble"),
                [("relations", output("relations", "result"))],
            ),
        ],
        output("assembled", "result"),
    );
    spec.inputs.insert("statement".into(), ValueType::Text);
    spec.outputs
        .insert("actors".into(), output("actors", "result"));
    spec.outputs
        .insert("actions".into(), output("actions", "result"));
    spec.subgraphs = BTreeMap::from([
        ("roles".into(), roles),
        ("masks".into(), masks),
        ("relations".into(), relations),
    ]);
    (spec, registry)
}
struct SemanticDouble {
    expected: BTreeSet<(String, String)>,
    requests: Mutex<Vec<ModelRequest>>,
}
#[async_trait::async_trait]
impl ModelBackend for SemanticDouble {
    async fn infer(&self, r: &ModelRequest) -> Result<ModelResponse> {
        self.requests.lock().unwrap().push(r.clone());
        let q = &r.questions[0];
        let answer = match &q.question {
            Question::Choice { options, .. } => {
                let surface = text_field(&r.state, "surface");
                let selected = if ["client", "service"].contains(&surface.as_str()) {
                    "actor"
                } else {
                    "action"
                };
                Answer::Choice {
                    selected: selected.into(),
                    confidence: probability(0.9),
                    probabilities: Some(
                        options
                            .iter()
                            .map(|o| {
                                (
                                    o.label.clone(),
                                    probability(if o.label == selected { 0.9 } else { 0.05 }),
                                )
                            })
                            .collect(),
                    ),
                }
            }
            Question::Boolean { .. } => Answer::Boolean {
                probability: probability(
                    if self.expected.contains(&(
                        text_field(&r.state, "actor"),
                        text_field(&r.state, "action"),
                    )) {
                        0.95
                    } else {
                        0.05
                    },
                ),
            },
            Question::Score { .. } => panic!("unexpected rubric"),
        };
        Ok(ModelResponse {
            model: r.model.clone(),
            raw: None,
            answers: BTreeMap::from([(q.id.clone(), answer)]),
            usage: None,
        })
    }
}
/// Trace: FR-002-AC-1, FR-002-AC-2, FR-003-AC-1, FR-004-AC-3, FR-010-AC-1, FR-012-AC-3, FR-014-AC-1, FR-017-AC-2, FR-020-AC-1
#[tokio::test]
async fn consumer_composes_extraction_roles_pairs_dependent_questions_and_assembly() {
    let word_ids = |word: &str| {
        STATEMENT
            .match_indices(word)
            .map(|(offset, _)| format!("word-{offset}"))
            .collect::<Vec<_>>()
    };
    let clients = word_ids("client");
    let services = word_ids("service");
    let expected = BTreeSet::from([
        (clients[0].clone(), word_ids("uploads")[0].clone()),
        (services[0].clone(), word_ids("store")[0].clone()),
        (services[1].clone(), word_ids("sends")[0].clone()),
        (clients[1].clone(), word_ids("display")[0].clone()),
    ]);
    let backend = Arc::new(SemanticDouble {
        expected: expected.clone(),
        requests: Mutex::new(Vec::new()),
    });
    let (spec, natives) = definitions();
    let engine = engine(&spec, &natives, bindings(backend.clone()));
    let run = engine
        .run(
            &BTreeMap::from([(
                "statement".into(),
                datum("statement", Value::Text(STATEMENT.into())),
            )]),
            limits(),
        )
        .await
        .unwrap();
    for port in ["actors", "actions"] {
        let Value::List(items) = &run.outputs[port].value else {
            panic!("collection expected")
        };
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| !item.sources.is_empty()));
    }
    let Value::List(edges) = &run.outputs["result"].value else {
        panic!("edges expected")
    };
    assert_eq!(edges.len(), 4);
    assert_eq!(
        edges
            .iter()
            .map(|item| (
                text_field(&item.value, "actor"),
                text_field(&item.value, "action")
            ))
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert!(edges.iter().all(|edge| edge.sources.len() >= 2));
    let requests = backend.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 24);
    assert!(
        requests
            .iter()
            .all(|r| text_field(&r.state, "context") == STATEMENT)
    );
    assert_eq!(requests.iter().filter(|r|matches!(&r.questions[0].question,Question::Boolean{instructions,..} if instructions=="Does this actor perform this action in the statement?")).count(),16);
    let empty = engine
        .run(
            &BTreeMap::from([(
                "statement".into(),
                datum("empty", Value::Text(String::new())),
            )]),
            limits(),
        )
        .await
        .unwrap();
    assert_eq!(empty.outputs["result"].value, Value::List(vec![]));
    assert_eq!(backend.requests.lock().unwrap().len(), 24);
}
