// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Public-interface test helpers with doubles only at native/model seams.
use sapho::{core::*, graph::*, runtime::*};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

pub fn datum(id: &str, value: Value) -> Datum {
    Datum::new(id, value).unwrap()
}
pub fn lit(value: Value, ty: ValueType) -> Binding {
    Binding::Literal {
        value: datum("literal", value),
        value_type: ty,
    }
}
pub fn input(name: &str) -> Binding {
    Binding::Input {
        name: name.into(),
        path: vec![],
    }
}
pub fn output(node: &str, port: &str) -> Binding {
    Binding::Node {
        node: NodeId::new(node).unwrap(),
        port: port.into(),
        path: vec![],
    }
}
pub fn node(
    id: &str,
    operation: Operation,
    inputs: impl IntoIterator<Item = (&'static str, Binding)>,
) -> NodeSpec {
    NodeSpec {
        id: NodeId::new(id).unwrap(),
        operation,
        inputs: inputs.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        guard: None,
    }
}
pub fn graph(nodes: Vec<NodeSpec>, out: Binding) -> GraphSpec {
    GraphSpec {
        inputs: BTreeMap::new(),
        nodes,
        outputs: BTreeMap::from([("result".into(), out)]),
        subgraphs: BTreeMap::new(),
    }
}
pub fn limits() -> RunLimits {
    RunLimits {
        node_instances: 2000,
        collection_items: 1000,
        model_requests: 100,
        concurrency: 4,
        data_bytes: 4_000_000,
        duration: Duration::from_secs(10),
    }
}
pub fn engine(
    spec: &GraphSpec,
    primitives: &PrimitiveRegistry,
    backends: BackendRegistry,
) -> Engine {
    Engine::new(compile(spec, primitives).unwrap(), backends).unwrap()
}
pub async fn operation(op: Operation, args: Vec<(&'static str, Value, ValueType)>) -> Value {
    let spec = graph(
        vec![node(
            "op",
            op,
            args.into_iter().map(|(k, v, t)| (k, lit(v, t))),
        )],
        output("op", "result"),
    );
    engine(
        &spec,
        &PrimitiveRegistry::default(),
        BackendRegistry::default(),
    )
    .run(&Inputs::new(), limits())
    .await
    .unwrap()
    .outputs["result"]
        .value
        .clone()
}
pub fn list(values: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::List(values.into_iter().map(|(id, v)| datum(id, v)).collect())
}
pub fn record(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Record(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn record_type(fields: impl IntoIterator<Item = (&'static str, ValueType)>) -> ValueType {
    ValueType::Record {
        fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    }
}
pub fn probability(v: f64) -> Probability {
    Probability::new(v).unwrap()
}
pub fn degree(v: f64) -> Value {
    Value::Degree(Degree::new(v).unwrap())
}
pub fn question() -> NamedQuestion {
    NamedQuestion {
        id: "q".into(),
        question: Question::Boolean {
            instructions: "Does the declared property hold?".into(),
            yes: "The property holds".into(),
            no: "The property does not hold".into(),
        },
    }
}
pub fn ask_graph(guard: Option<bool>) -> GraphSpec {
    let q = node(
        "q",
        Operation::Questions {
            questions: vec![question()],
        },
        [],
    );
    let mut ask = node(
        "ask",
        Operation::Ask {
            backend: BackendId::new("judge").unwrap(),
        },
        [
            ("state", lit(record([]), record_type([]))),
            ("questions", output("q", "result")),
        ],
    );
    ask.guard = guard.map(|g| lit(Value::Boolean(g), ValueType::Boolean));
    graph(vec![q, ask], output("ask", "answers"))
}
pub fn bindings(backend: Arc<dyn ModelBackend>) -> BackendRegistry {
    let mut b = BackendRegistry::default();
    b.register(
        BackendId::new("judge").unwrap(),
        BackendBinding {
            distribution_policy: sapho::core::DistributionPolicy::Strict {},
            backend,
            model: "model-1".into(),
            expected_model: Some("model-1".into()),
        },
    )
    .unwrap();
    b
}

pub type NativeFn = dyn Fn(&Inputs, &BTreeMap<String, Value>) -> Result<Inputs> + Send + Sync;
pub struct Native {
    pub signature: Signature,
    pub f: Box<NativeFn>,
}
impl Primitive for Native {
    fn signature(&self) -> Signature {
        self.signature.clone()
    }
    fn execute(
        &self,
        ctx: &PrimitiveContext,
        input: &Inputs,
        params: &BTreeMap<String, Value>,
    ) -> Result<Inputs> {
        ctx.check_cancelled()?;
        (self.f)(input, params)
    }
}
pub struct Scripted {
    pub calls: AtomicUsize,
    pub answer: f64,
}
#[async_trait::async_trait]
impl ModelBackend for Scripted {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse {
            model: request.model.clone(),
            raw: None,
            answers: request
                .questions
                .iter()
                .map(|q| {
                    let a = match &q.question {
                        Question::Boolean { .. } => Answer::Boolean {
                            probability: probability(self.answer),
                        },
                        Question::Choice { options, .. } => Answer::Choice {
                            selected: options[0].label.clone(),
                            confidence: probability(self.answer),
                            probabilities: Some(
                                options
                                    .iter()
                                    .enumerate()
                                    .map(|(i, o)| {
                                        (
                                            o.label.clone(),
                                            probability(if i == 0 {
                                                self.answer
                                            } else {
                                                (1.0 - self.answer)
                                                    / f64::from(
                                                        u32::try_from(options.len() - 1).unwrap(),
                                                    )
                                            }),
                                        )
                                    })
                                    .collect(),
                            ),
                        },
                        Question::Score { .. } => Answer::Score {
                            expected: 0.0,
                            confidence: probability(self.answer),
                            probabilities: None,
                        },
                    };
                    (q.id.clone(), a)
                })
                .collect(),
            usage: None,
        })
    }
}
