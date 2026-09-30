---
id: FR-019
title: "Compare facts explicitly"
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
# FR-019: Compare facts explicitly

## Description

The Sapho comparison evaluator SHALL compare compatible scalar values using the configured operator.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

equal compares operands of the same scalar type: Text, Boolean, Number, Probability or Degree. less/less_equal/greater/greater_equal accept same-kind numeric scalars, never implicit Probability/Degree interchange. Output is Boolean. All numbers are finite. Comparison at equality is exact, with no hidden epsilon; distribution mass validation is the separate stated tolerance.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-019-AC-1 | Equality and ordered comparisons return the expected results below, at and above a cutoff. | Test (TC-019) |
| FR-019-AC-2 | Text ordering, mixed numeric kinds and optional operands are rejected. | Test (TC-019) |
| FR-019-AC-3 | A degree exactly equal to a greater_equal cutoff returns true. | Test (TC-019) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
