// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Pure dependency ordering, signature checking and native implementation binding.
use crate::{Binding, Comparator, GraphBody, GraphSpec, NodeSpec, Operation, Reducer};
use sapho_core::{
    ErrorCode, NodeId, Primitive, PrimitiveRegistry, Result, SaphoError, Signature, ValueType,
    validate_name, validate_questions,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// An immutable graph ready for execution; no inference occurs during compilation.
#[derive(Clone)]
pub struct CompiledGraph {
    /// Input/output schema of this body.
    pub(crate) signature: Signature,
    /// Stable topological stages, each in declaration order.
    pub(crate) stages: Vec<Vec<CompiledNode>>,
    /// Named graph output bindings.
    pub(crate) outputs: BTreeMap<String, Binding>,
    map_depth: usize,
}
/// A checked node with its original operation and captured native implementation.
#[derive(Clone)]
pub struct CompiledNode {
    /// Original declarative node for execution evidence.
    pub(crate) spec: NodeSpec,
    /// Unwrapped operation signature; guards wrap only the produced port values.
    pub(crate) signature: Signature,
    /// Exposed port types including guard Optional wrappers.
    pub(crate) output_types: BTreeMap<String, ValueType>,
    /// Bound host-native implementation, present only on code nodes.
    pub(crate) primitive: Option<Arc<dyn Primitive>>,
    /// Bound reusable graph, present only on map nodes.
    pub(crate) mapped_graph: Option<Arc<CompiledGraph>>,
}
/// Compile every definition and the root graph without evaluating code or models.
pub fn compile(spec: &GraphSpec, registry: &PrimitiveRegistry) -> Result<CompiledGraph> {
    let total = spec.subgraphs.values().try_fold(spec.nodes.len(), |n, g| {
        n.checked_add(g.nodes.len())
            .ok_or_else(|| limit("Node count overflow"))
    })?;
    if total > 4096 {
        return Err(limit("Graph definitions exceed 4096 nodes"));
    }
    let mut c = Compiler {
        registry,
        defs: &spec.subgraphs,
        cache: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    for name in spec.subgraphs.keys() {
        c.subgraph(name, 0)?;
    }
    c.body(
        &GraphBody {
            inputs: spec.inputs.clone(),
            nodes: spec.nodes.clone(),
            outputs: spec.outputs.clone(),
        },
        0,
    )
}
struct Compiler<'a> {
    registry: &'a PrimitiveRegistry,
    defs: &'a BTreeMap<String, GraphBody>,
    cache: BTreeMap<String, Arc<CompiledGraph>>,
    active: BTreeSet<String>,
}
impl Compiler<'_> {
    fn subgraph(&mut self, name: &str, depth: usize) -> Result<Arc<CompiledGraph>> {
        if depth >= 16 {
            return Err(limit("Subgraph depth exceeds 16"));
        }
        if self.active.contains(name) {
            return Err(SaphoError::new(
                ErrorCode::Cycle,
                "Recursive subgraph reference",
            ));
        }
        if let Some(g) = self.cache.get(name) {
            if depth + g.map_depth > 16 {
                return Err(limit("Subgraph depth exceeds 16"));
            }
            return Ok(g.clone());
        }
        validate_name(name)?;
        let body = self
            .defs
            .get(name)
            .cloned()
            .ok_or_else(|| unknown("Subgraph absent", name))?;
        self.active.insert(name.into());
        let g = self.body(&body, depth + 1)?;
        self.active.remove(name);
        let g = Arc::new(g);
        self.cache.insert(name.into(), g.clone());
        Ok(g)
    }
    fn body(&mut self, body: &GraphBody, depth: usize) -> Result<CompiledGraph> {
        for (name, t) in &body.inputs {
            validate_name(name)?;
            t.validate()?;
        }
        let mut ids = BTreeSet::new();
        for n in &body.nodes {
            validate_name(n.id.as_str())?;
            if !ids.insert(n.id.clone()) {
                return Err(SaphoError::new(
                    ErrorCode::DuplicateId,
                    "Repeated node identity",
                ));
            }
        }
        let mut dependencies = BTreeMap::new();
        for n in &body.nodes {
            let ds = n
                .inputs
                .values()
                .chain(n.guard.iter())
                .filter_map(|b| match b {
                    Binding::Node { node, .. } => Some(node.clone()),
                    Binding::Input { .. } | Binding::Literal { .. } => None,
                })
                .collect::<BTreeSet<_>>();
            if let Some(id) = ds.iter().find(|id| !ids.contains(*id)) {
                return Err(unknown("Producer node absent", id.as_str()));
            }
            dependencies.insert(n.id.clone(), ds);
        }
        let mut done = BTreeSet::new();
        let mut types = BTreeMap::new();
        let mut stages = Vec::new();
        while done.len() < body.nodes.len() {
            let ready = body
                .nodes
                .iter()
                .filter(|n| {
                    !done.contains(&n.id)
                        && dependencies
                            .get(&n.id)
                            .is_some_and(|ds| ds.is_subset(&done))
                })
                .collect::<Vec<_>>();
            if ready.is_empty() {
                return Err(SaphoError::new(ErrorCode::Cycle, "Node dependency cycle"));
            }
            let mut stage = Vec::new();
            for n in ready {
                let incoming = n
                    .inputs
                    .iter()
                    .map(|(k, b)| {
                        validate_name(k)?;
                        Ok((k.clone(), binding_type(b, &body.inputs, &types)?))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()?;
                if let Some(g) = &n.guard
                    && binding_type(g, &body.inputs, &types)? != ValueType::Boolean
                {
                    return Err(mismatch("Guard must be Boolean"));
                }
                let (signature, primitive, mapped_graph) =
                    self.operation(&n.operation, &incoming, depth)?;
                if signature.inputs != incoming {
                    return Err(mismatch("Node operands differ from operation signature")
                        .with_context("node", n.id.to_string()));
                }
                for (name, t) in signature.inputs.iter().chain(&signature.outputs) {
                    validate_name(name)?;
                    t.validate()?;
                }
                let output_types = signature
                    .outputs
                    .iter()
                    .map(|(k, t)| {
                        (
                            k.clone(),
                            if n.guard.is_some() {
                                ValueType::optional(t.clone())
                            } else {
                                t.clone()
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                types.insert(n.id.clone(), output_types.clone());
                stage.push(CompiledNode {
                    spec: n.clone(),
                    signature,
                    output_types,
                    primitive,
                    mapped_graph,
                });
            }
            for n in &stage {
                done.insert(n.spec.id.clone());
            }
            stages.push(stage);
        }
        let outputs = body
            .outputs
            .iter()
            .map(|(name, b)| {
                validate_name(name)?;
                Ok((name.clone(), binding_type(b, &body.inputs, &types)?))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        if outputs.is_empty() {
            return Err(SaphoError::new(
                ErrorCode::Config,
                "Graph has no named outputs",
            ));
        }
        let map_depth = stages
            .iter()
            .flatten()
            .filter_map(|n| n.mapped_graph.as_ref())
            .map(|g| g.map_depth + 1)
            .max()
            .unwrap_or(0);
        if depth + map_depth > 16 {
            return Err(limit("Subgraph depth exceeds 16"));
        }
        Ok(CompiledGraph {
            map_depth,
            signature: Signature {
                inputs: body.inputs.clone(),
                outputs,
            },
            stages,
            outputs: body.outputs.clone(),
        })
    }
    fn operation(
        &mut self,
        op: &Operation,
        ins: &BTreeMap<String, ValueType>,
        depth: usize,
    ) -> Result<OperationSignature> {
        let mut primitive = None;
        let mut mapped = None;
        let (inputs, outputs) = match op {
            Operation::Record {} => (
                ins.clone(),
                one(
                    "result",
                    ValueType::Record {
                        fields: ins.clone(),
                    },
                ),
            ),
            Operation::List { item_type, order } => {
                item_type.validate()?;
                let names = order.iter().collect::<BTreeSet<_>>();
                if names.len() != order.len() || !names.iter().copied().eq(ins.keys()) {
                    return Err(SaphoError::new(
                        ErrorCode::Config,
                        "List order must name each operand exactly once",
                    ));
                }
                for name in order {
                    validate_name(name)?;
                }
                if ins.values().any(|ty| ty != item_type) {
                    return Err(mismatch("List operands must match item_type"));
                }
                (
                    ins.clone(),
                    one("result", ValueType::list(item_type.clone())),
                )
            }
            Operation::Code {
                primitive: id,
                params,
            } => {
                for value in params.values() {
                    value.validate()?;
                }
                let p = self.registry.get(id)?;
                let sig = p.signature();
                primitive = Some(p);
                (sig.inputs, sig.outputs)
            }
            Operation::Questions { questions } => {
                validate_questions(questions)?;
                (BTreeMap::new(), one("result", ValueType::Questions))
            }
            Operation::Ask { backend } => {
                backend.validate()?;
                let state = operand(ins, "state")?.clone();
                if !matches!(state, ValueType::Record { .. }) {
                    return Err(mismatch("Ask state must be Record"));
                }
                (
                    BTreeMap::from([
                        ("state".into(), state),
                        ("questions".into(), ValueType::Questions),
                    ]),
                    BTreeMap::from([
                        ("answers".into(), ValueType::Answers),
                        ("model".into(), ValueType::Text),
                    ]),
                )
            }
            Operation::Map { graph } => {
                let t = list_item(operand(ins, "items")?)?.clone();
                let g = self.subgraph(graph, depth)?;
                if g.signature.inputs.get("item") != Some(&t)
                    || g.signature.outputs.len() != 1
                    || !g.signature.outputs.contains_key("result")
                {
                    return Err(mismatch(
                        "Mapped subgraph needs matching item input and only result output",
                    ));
                }
                let mut expected = g.signature.inputs.clone();
                expected.remove("item");
                expected.insert("items".into(), ValueType::list(t));
                let result = g
                    .signature
                    .outputs
                    .get("result")
                    .cloned()
                    .ok_or_else(|| unknown("Map output absent", "result"))?;
                mapped = Some(g);
                (expected, one("result", ValueType::list(result)))
            }
            Operation::Filter => {
                let items = operand(ins, "items")?.clone();
                list_item(&items)?;
                (
                    BTreeMap::from([
                        ("items".into(), items.clone()),
                        ("mask".into(), ValueType::list(ValueType::Boolean)),
                    ]),
                    one("result", items),
                )
            }
            Operation::Pairs | Operation::Join { .. } => {
                let left = operand(ins, "left")?.clone();
                let right = operand(ins, "right")?.clone();
                let l = list_item(&left)?.clone();
                let rr = list_item(&right)?.clone();
                if let Operation::Join {
                    left_key,
                    right_key,
                } = op
                {
                    let lt = field_type(&l, left_key)?;
                    let rt = field_type(&rr, right_key)?;
                    if lt != rt
                        || !matches!(lt, ValueType::Text | ValueType::Boolean | ValueType::Number)
                    {
                        return Err(mismatch("Join keys must have the same scalar fact type"));
                    }
                }
                (
                    BTreeMap::from([("left".into(), left), ("right".into(), right)]),
                    one(
                        "result",
                        ValueType::list(ValueType::Record {
                            fields: BTreeMap::from([("left".into(), l), ("right".into(), rr)]),
                        }),
                    ),
                )
            }
            Operation::Collect => {
                let items = operand(ins, "items")?.clone();
                let inner = list_item(&items)?.clone();
                list_item(&inner)?;
                (one("items", items), one("result", inner))
            }
            Operation::And | Operation::Or => (
                BTreeMap::from([
                    ("a".into(), ValueType::Boolean),
                    ("b".into(), ValueType::Boolean),
                ]),
                one("result", ValueType::Boolean),
            ),
            Operation::Not => (
                one("value", ValueType::Boolean),
                one("result", ValueType::Boolean),
            ),
            Operation::Compare { comparator } => {
                let a = operand(ins, "a")?.clone();
                let b = operand(ins, "b")?.clone();
                let allowed = match comparator {
                    Comparator::Equal => matches!(
                        a,
                        ValueType::Boolean
                            | ValueType::Text
                            | ValueType::Number
                            | ValueType::Probability
                            | ValueType::Degree
                    ),
                    Comparator::Less
                    | Comparator::LessEqual
                    | Comparator::Greater
                    | Comparator::GreaterEqual => matches!(
                        a,
                        ValueType::Number | ValueType::Probability | ValueType::Degree
                    ),
                };
                if a != b || !allowed {
                    return Err(mismatch("Comparison requires compatible scalar operands"));
                }
                (
                    BTreeMap::from([("a".into(), a), ("b".into(), b)]),
                    one("result", ValueType::Boolean),
                )
            }
            Operation::Probability { question, labels } => {
                if question.is_empty()
                    || labels.is_empty()
                    || labels.iter().any(|s| s.is_empty())
                    || labels.iter().collect::<BTreeSet<_>>().len() != labels.len()
                {
                    return Err(SaphoError::new(
                        ErrorCode::Config,
                        "Invalid probability projection",
                    ));
                }
                (
                    one("answers", ValueType::Answers),
                    one("result", ValueType::Probability),
                )
            }
            Operation::Degree => {
                let t = operand(ins, "value")?.clone();
                if !matches!(t, ValueType::Probability | ValueType::Number) {
                    return Err(mismatch("Degree requires Probability or Number"));
                }
                (one("value", t), one("result", ValueType::Degree))
            }
            Operation::Reduce { reducer, .. } => {
                let mut i = one("values", ValueType::list(ValueType::Degree));
                if *reducer == Reducer::WeightedMean {
                    i.insert("weights".into(), ValueType::list(ValueType::Number));
                }
                (i, one("result", ValueType::Degree))
            }
            Operation::Complement => (
                one("value", ValueType::Degree),
                one("result", ValueType::Degree),
            ),
            Operation::Coalesce => {
                let optional = operand(ins, "value")?.clone();
                let ValueType::Optional { inner } = optional.clone() else {
                    return Err(mismatch("Coalesce requires Optional input"));
                };
                (
                    BTreeMap::from([
                        ("value".into(), optional),
                        ("default".into(), *inner.clone()),
                    ]),
                    one("result", *inner),
                )
            }
            Operation::MergePresent {} => {
                if !(2..=32).contains(&ins.len()) {
                    return Err(SaphoError::new(
                        ErrorCode::Config,
                        "MergePresent needs two to 32 operands",
                    ));
                }
                let inner = ins
                    .values()
                    .next()
                    .and_then(|ty| match ty {
                        ValueType::Optional { inner } => Some(inner.as_ref()),
                        _ => None,
                    })
                    .ok_or_else(|| mismatch("MergePresent needs Optional operands"))?;
                if ins
                    .values()
                    .any(|ty| ty != &ValueType::optional(inner.clone()))
                {
                    return Err(mismatch("MergePresent operands need one exact inner type"));
                }
                (ins.clone(), one("result", inner.clone()))
            }
        };
        Ok((Signature { inputs, outputs }, primitive, mapped))
    }
}
type OperationSignature = (
    Signature,
    Option<Arc<dyn Primitive>>,
    Option<Arc<CompiledGraph>>,
);
fn one(name: &str, t: ValueType) -> BTreeMap<String, ValueType> {
    BTreeMap::from([(name.into(), t)])
}
fn operand<'a>(ins: &'a BTreeMap<String, ValueType>, key: &str) -> Result<&'a ValueType> {
    ins.get(key).ok_or_else(|| {
        SaphoError::new(ErrorCode::MissingInput, "Operation operand absent")
            .with_context("port", key)
    })
}
fn list_item(t: &ValueType) -> Result<&ValueType> {
    if let ValueType::List { item } = t {
        Ok(item)
    } else {
        Err(mismatch("Expected list port"))
    }
}
fn field_type<'a>(t: &'a ValueType, name: &str) -> Result<&'a ValueType> {
    validate_name(name)?;
    if let ValueType::Record { fields } = t {
        fields
            .get(name)
            .ok_or_else(|| unknown("Record field absent", name))
    } else {
        Err(mismatch("Expected record port"))
    }
}
fn binding_type(
    b: &Binding,
    inputs: &BTreeMap<String, ValueType>,
    nodes: &BTreeMap<NodeId, BTreeMap<String, ValueType>>,
) -> Result<ValueType> {
    let (mut t, path) = match b {
        Binding::Input { name, path } => (
            inputs
                .get(name)
                .cloned()
                .ok_or_else(|| unknown("Graph input absent", name))?,
            path,
        ),
        Binding::Node { node, port, path } => (
            nodes
                .get(node)
                .and_then(|ports| ports.get(port))
                .cloned()
                .ok_or_else(|| unknown("Producer port absent", port))?,
            path,
        ),
        Binding::Literal { value, value_type } => {
            value.validate()?;
            value_type.check(&value.value)?;
            return Ok(value_type.clone());
        }
    };
    if path.len() > 32 {
        return Err(limit("Binding projection exceeds 32"));
    }
    for field in path {
        t = field_type(&t, field)?.clone();
    }
    Ok(t)
}
fn unknown(message: &str, name: &str) -> SaphoError {
    SaphoError::new(ErrorCode::UnknownReference, message).with_context("name", name)
}
fn mismatch(message: &str) -> SaphoError {
    SaphoError::new(ErrorCode::TypeMismatch, message)
}
fn limit(message: &str) -> SaphoError {
    SaphoError::new(ErrorCode::LimitExceeded, message)
}

impl CompiledGraph {
    /// Borrow the compiled input/output contract.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
    /// Borrow immutable topological stages.
    pub fn stages(&self) -> &[Vec<CompiledNode>] {
        &self.stages
    }
    /// Borrow checked output bindings.
    pub fn outputs(&self) -> &BTreeMap<String, Binding> {
        &self.outputs
    }
}
impl CompiledNode {
    /// Borrow the original declarative operation.
    pub fn spec(&self) -> &NodeSpec {
        &self.spec
    }
    /// Borrow the captured operation signature.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
    /// Borrow exposed output types, including guard wrapping.
    pub fn output_types(&self) -> &BTreeMap<String, ValueType> {
        &self.output_types
    }
    /// Clone the implementation bound during compilation.
    pub fn primitive(&self) -> Option<Arc<dyn Primitive>> {
        self.primitive.clone()
    }
    /// Clone the reusable graph bound during compilation.
    pub fn mapped_graph(&self) -> Option<Arc<CompiledGraph>> {
        self.mapped_graph.clone()
    }
}
