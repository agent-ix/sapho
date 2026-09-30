---
id: FR-022
title: "Combine heuristic degrees"
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
  - target: "ix://agent-ix/sapho/FR-021"
    type: depends_on
---
# FR-022: Combine heuristic degrees

## Description

The Sapho degree reducer SHALL apply the selected min, max or weighted-mean formula to the supplied degrees.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Inputs values: List(Degree). Min/max choose extrema. Weighted_mean additionally receives weights: List(Number), aligned by item ID with values, finite and non-negative; at least one weight is positive for a non-empty input. Weighted mean is sum(w*x)/sum(w), evaluated with weight normalization to avoid overflow. Every reduction declares an empty Degree parameter, used only when values are empty. Reject missing/extra/duplicate weights and zero total weight on non-empty values. Output is Degree, never Probability.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-022-AC-1 | Values 0.2 and 0.8 give min 0.2, max 0.8 and equal-weight mean 0.5. | Test (TC-022) |
| FR-022-AC-2 | Empty values use the explicit empty degree; invalid/misaligned weights fail. | Test (TC-022) |
| FR-022-AC-3 | Large finite weights yield a finite unit-range mean without overflow. | Test (TC-022) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
- [FR-021](FR-021-convert-evidence-to-heuristic-degree.md)
