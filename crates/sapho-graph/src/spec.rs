// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Configuration vocabulary; no domain rules or inference execution.
use sapho_core::{
    BackendId, Datum, Degree, ErrorCode, NamedQuestion, NodeId, PrimitiveId, Result, SaphoError,
    Value, ValueType,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A typed connection with an optional record-field projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Binding {
    /// A declared graph input.
    Input {
        /// Input port name.
        name: String,
        /// Nested record fields to select.
        #[serde(default)]
        path: Vec<String>,
    },
    /// A named output of another node in this graph body.
    Node {
        /// Producer node.
        node: NodeId,
        /// Producer port.
        port: String,
        /// Nested record fields to select.
        #[serde(default)]
        path: Vec<String>,
    },
    /// A checked literal with an explicit type, including empty collections.
    Literal {
        /// Complete value and sources.
        value: Datum,
        /// Declared literal schema.
        value_type: ValueType,
    },
}
/// Explicit scalar comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    /// Exact equality.
    Equal,
    /// Strictly less.
    Less,
    /// Less or equal.
    LessEqual,
    /// Strictly greater.
    Greater,
    /// Greater or equal.
    GreaterEqual,
}
/// Explicit unit-degree reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reducer {
    /// Minimum.
    Min,
    /// Maximum.
    Max,
    /// Weight-normalized arithmetic mean.
    WeightedMean,
}
/// Registered code, inference, collection and logic operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Invoke host-native Rust code.
    Code {
        /// Registered implementation name.
        primitive: PrimitiveId,
        /// Typed application-owned parameters.
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    /// Construct a literal ordered question block.
    Questions {
        /// Consumer question definitions.
        questions: Vec<NamedQuestion>,
    },
    /// One explicitly batched model call.
    Ask {
        /// Host-configured model binding.
        backend: BackendId,
    },
    /// Map a reusable subgraph over identified items.
    Map {
        /// Subgraph name in the flat definition collection.
        graph: String,
    },
    /// Filter using identified Boolean masks.
    Filter,
    /// Cartesian left/right pairs.
    Pairs,
    /// Inner many-to-many equality join.
    Join {
        /// Scalar field in left records.
        left_key: String,
        /// Scalar field in right records.
        right_key: String,
    },
    /// Flatten one collection nesting level.
    Collect,
    /// Boolean conjunction.
    And,
    /// Boolean disjunction.
    Or,
    /// Boolean negation.
    Not,
    /// Compare same-kind scalar facts.
    Compare {
        /// Mathematical comparator.
        comparator: Comparator,
    },
    /// Project specified model outcome mass.
    Probability {
        /// Question ID.
        question: String,
        /// Non-empty distinct outcome labels.
        labels: Vec<String>,
    },
    /// Explicit unit-range conversion into a heuristic degree.
    Degree,
    /// Combine an identified degree collection.
    Reduce {
        /// Formula to apply.
        reducer: Reducer,
        /// Caller-chosen value for an empty collection.
        empty: Degree,
    },
    /// One minus a heuristic degree.
    Complement,
    /// Explicit replacement of Optional absence.
    Coalesce,
}
/// One named node with typed connections and an optional execution guard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    /// Identity unique within the graph body.
    pub id: NodeId,
    /// Closed operation or named native implementation.
    pub operation: Operation,
    /// Named operand bindings.
    #[serde(default)]
    pub inputs: BTreeMap<String, Binding>,
    /// Boolean guard; false performs no work and returns Optional absence.
    pub guard: Option<Binding>,
}
/// A reusable graph body, independent of the enclosing graph's inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphBody {
    /// Named input schemas.
    #[serde(default)]
    pub inputs: BTreeMap<String, ValueType>,
    /// Declared node order, retained as the stable dependency tie-break.
    #[serde(default)]
    pub nodes: Vec<NodeSpec>,
    /// Named output connections.
    pub outputs: BTreeMap<String, Binding>,
}
/// Entire consumer program plus reusable named subgraph definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSpec {
    /// Root input schemas.
    #[serde(default)]
    pub inputs: BTreeMap<String, ValueType>,
    /// Root nodes in declaration order.
    #[serde(default)]
    pub nodes: Vec<NodeSpec>,
    /// Root named output connections.
    pub outputs: BTreeMap<String, Binding>,
    /// Flat reusable definitions; references are checked for recursion.
    #[serde(default)]
    pub subgraphs: BTreeMap<String, GraphBody>,
}
impl GraphSpec {
    /// Load TOML after applying a one-MiB pre-deserialization ceiling.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 1_048_576 {
            return Err(SaphoError::new(
                ErrorCode::LimitExceeded,
                "TOML exceeds one MiB",
            ));
        }
        toml::from_str(text).map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))
    }
}
