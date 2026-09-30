---
id: FR-018
title: "Evaluate crisp Boolean operations"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-004"
    type: implements
  - target: "ix://agent-ix/sapho/StR-004"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-018: Evaluate crisp Boolean operations

## Description

The Sapho logic evaluator SHALL evaluate Boolean conjunction, disjunction and negation according to their truth tables.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

and/or inputs are a,b: Boolean; not input is value: Boolean. Output result is Boolean. Optional and numeric inputs are type errors; unknown does not become false. No short-circuiting suppresses already configured upstream work; use guards to suppress node execution.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-018-AC-1 | All four AND/OR input permutations and both NOT inputs match their truth tables. | Test (TC-018) |
| FR-018-AC-2 | Number, Degree and Optional cannot bind to Boolean operands. | Test (TC-018) |
| FR-018-AC-3 | Boolean false remains distinct from absent Optional. | Test (TC-018) |

## Dependencies

- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
