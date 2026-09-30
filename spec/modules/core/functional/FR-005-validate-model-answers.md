---
id: FR-005
title: "Validate model answers"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-004"
    type: depends_on
---
# FR-005: Validate model answers

## Description

The Sapho answer boundary SHALL validate each model response against the questions that were sent.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

The response contains actual model identity, answers keyed by requested ID and optional input/output usage. Boolean answers contain P(true). Choice answers contain selected label, reported confidence and optional/partial label probabilities. Score answers contain finite expected score in [0,level_count-1], reported confidence and optional/partial level probabilities. Confidence and probabilities are finite in [0,1]. Extra/missing IDs, wrong answer types and unknown labels/levels are errors. A complete distribution sums to one within 1e-6; a partial distribution has mass <= 1+1e-6 and remains partial; missing entries are never filled or renormalized. Distribution state is Complete, Partial or Unavailable. Actual model identity is non-empty; an expected identity is checked only when the caller configures one.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-005-AC-1 | A valid mixed response preserves expected score, option probabilities, distribution completeness and actual model identity. | Test (TC-005) |
| FR-005-AC-2 | Missing/extra IDs, mismatched types, unknown labels, invalid mass and non-finite values return InvalidAnswer or MissingAnswer. | Test (TC-005) |
| FR-005-AC-3 | A partial or unavailable distribution remains distinguishable and never becomes a zero-valued complete distribution. | Test (TC-005) |

## Dependencies

- [FR-004](FR-004-declare-typed-questions.md)
