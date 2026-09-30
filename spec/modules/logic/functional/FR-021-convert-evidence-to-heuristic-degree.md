---
id: FR-021
title: "Convert evidence to heuristic degree"
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
# FR-021: Convert evidence to heuristic degree

## Description

The Sapho degree operator SHALL explicitly convert a unit-range scalar into a Degree.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Input value may be Probability or finite Number in [0,1]. Output result is Degree, retaining source/dependency evidence. Conversion indicates how the consumer elects to use the value; it does not claim calibration or truth. Out-of-range Number is InvalidValue; Answer and Boolean do not implicitly convert.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-021-AC-1 | Probability 0.7 converts to Degree 0.7 while trace retains its originating answer. | Test (TC-021) |
| FR-021-AC-2 | Numbers -0.01 and 1.01 are rejected. | Test (TC-021) |
| FR-021-AC-3 | The resulting value serializes as Degree rather than Probability. | Test (TC-021) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
