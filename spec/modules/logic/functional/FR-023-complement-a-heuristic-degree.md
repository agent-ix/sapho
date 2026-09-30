---
id: FR-023
title: "Complement a heuristic degree"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-004"
    type: implements
  - target: "ix://agent-ix/sapho/StR-004"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-023: Complement a heuristic degree

## Description

The Sapho complement evaluator SHALL return one minus the supplied Degree.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Input value: Degree; output result: Degree. This is the declared fuzzy complement; Probability complements require selecting the false outcome or explicit conversion first. Endpoint results are exact: 0 -> 1, 1 -> 0.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-023-AC-1 | Degree 0, 0.25 and 1 produce 1, 0.75 and 0. | Test (TC-023) |
| FR-023-AC-2 | A Probability input is rejected by graph compilation. | Test (TC-023) |
| FR-023-AC-3 | Source references and input identity remain inspectable in trace. | Test (TC-023) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
