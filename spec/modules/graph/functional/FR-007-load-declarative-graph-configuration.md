---
id: FR-007
title: "Load declarative graph configuration"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/US-002"
    type: implements
  - target: "ix://agent-ix/sapho/StR-002"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-003"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-004"
    type: depends_on
---
# FR-007: Load declarative graph configuration

## Description

The Sapho graph loader SHALL deserialize YAML or JSON into a typed graph definition.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

GraphSpec contains typed inputs, ordered named nodes, named output bindings and a flat named collection of reusable subgraphs. Each node contains id, an Operation, named input bindings and an optional Boolean guard binding. Bindings select a graph input, an earlier-or-later node port, or a typed literal; optional record-field paths are validated. Operation is a closed tagged enum, including code, questions, ask, map, filter, pairs, join, collect and the declared logic operators. Code operations name a primitive and carry typed params. Questions operations emit an ordered literal Questions value. Record/list assembly operations construct values from named typed bindings. A graph body has no scripts, credentials, file discovery, domain rule status or review severity fields. GraphSpec::parse(text) selects YAML; GraphSpec::parse_with_format(text, GraphFormat::Yaml or GraphFormat::Json) selects the explicit format. The graph crate performs no extension/path discovery. The command host chooses yaml/yml/json from the path or explicit flag. Graph TOML is removed without a compatibility loader. Both formats deserialize the same Serde-tagged schema, preserve sequence order and decoded string contents, and reject duplicate keys before map construction. YAML accepts only the constrained profile in NFR-005, including no aliases, merge keys, custom tags, include or property expansion. Ordinary syntax/shape refusals use Config; byte/parser budget refusal uses LimitExceeded. The loader rejects a YAML or JSON string larger than 1 MiB before deserialization. Type/value nesting is limited to 32, and graph-reference nesting to 16 during compilation. Unknown fields and malformed types are errors.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-007-AC-1 | Equivalent YAML and JSON graphs with literal questions, code nodes and named outputs produces the declared GraphSpec. | Test (TC-007) |
| FR-007-AC-2 | Unknown fields, malformed operation parameters and ambiguous binding shapes return Config. | Test (TC-007) |
| FR-007-AC-3 | A graph input or nested record field can be connected without losing its declared type. | Test (TC-007) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-003](../../core/functional/FR-003-register-custom-rust-primitives.md)
- [FR-004](../../core/functional/FR-004-declare-typed-questions.md)
