// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Full CLI feature graph must preserve FR-063 canonical semantic identity.
use sapho_graph::{GraphFormat, GraphSpec, graph_semantic_identity};

/// Trace: FR-063-AC-2, IT-009-SC-04
#[test]
fn reordered_yaml_and_json_named_maps_have_one_semantic_identity() {
    let yaml = r#"
outputs:
  result:
    kind: literal
    value:
      id: example
      value:
        kind: record
        value:
          b: {kind: number, value: 2}
          a: {kind: number, value: 1}
    value_type:
      kind: record
      fields:
        b: {kind: number}
        a: {kind: number}
"#;
    let json = r#"{"outputs":{"result":{"value_type":{"fields":{"a":{"kind":"number"},"b":{"kind":"number"}},"kind":"record"},"value":{"value":{"value":{"a":{"value":1,"kind":"number"},"b":{"value":2,"kind":"number"}},"kind":"record"},"id":"example"},"kind":"literal"}}}"#;
    let yaml = GraphSpec::parse_with_format(yaml, GraphFormat::Yaml).unwrap();
    let json = GraphSpec::parse_with_format(json, GraphFormat::Json).unwrap();
    assert_eq!(yaml, json);
    assert_eq!(
        graph_semantic_identity(&yaml).unwrap(),
        graph_semantic_identity(&json).unwrap()
    );
}
