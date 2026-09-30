---
id: FR-012
title: "Map reusable subgraphs"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-011"
    type: depends_on
---
# FR-012: Map reusable subgraphs

## Description

The Sapho map operation SHALL execute a named subgraph for each identified input item.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Map inputs contain items: List(T) and captures matching the subgraph inputs other than item: T. Each reusable mapped subgraph declares exactly one output named result. Map returns List(result_type), keeps original item IDs and order, and propagates each item plus captures sources. Nested maps share limits. An empty list emits an empty typed list and does not invoke the subgraph or a backend.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-012-AC-1 | A map transforms three records while preserving their three IDs, order and source spans. | Test (TC-012) |
| FR-012-AC-2 | An empty map produces no model requests. | Test (TC-012) |
| FR-012-AC-3 | Captured context and an earlier judgment reach a later mapped question through declared inputs. | Test (TC-012) |

## Dependencies

- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
- [FR-011](FR-011-enforce-run-resource-limits.md)
