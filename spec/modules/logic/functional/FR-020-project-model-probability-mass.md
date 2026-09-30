---
id: FR-020
title: "Project model probability mass"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-004"
    type: implements
  - target: "ix://agent-ix/sapho/StR-004"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-020: Project model probability mass

## Description

The Sapho probability operator SHALL sum only the requested answer outcomes from a validated answer.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Inputs answers: Answers; params question: ID and labels: non-empty unique strings. Boolean labels are true/false and derive P(true)/1-P(true). Choice labels name declared options; Score labels name zero-based decimal indices. Return Probability. Unknown labels are InvalidAnswer; missing question is MissingAnswer; a requested probability absent from a partial/unavailable distribution is UnsupportedDistribution. A known mass in a partial distribution can be returned; no renormalization or completeness claim is made. Sum using finite arithmetic and reject mass outside [0,1] beyond roundoff tolerance.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-020-AC-1 | Selecting one or several known labels yields their exact probability mass. | Test (TC-020) |
| FR-020-AC-2 | A missing probability in a partial answer returns UnsupportedDistribution. | Test (TC-020) |
| FR-020-AC-3 | A Boolean false projection returns one minus its true probability. | Test (TC-020) |

## Dependencies

- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
