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

The response contains actual model identity, an optional digest of the actual model's weights when the provider reports one, an optional raw exchange ([FR-006](FR-006-define-replaceable-model-backend.md)) that validation leaves unchanged, answers keyed by requested ID and optional input/output usage. Boolean answers contain P(true). Choice answers contain selected label, reported confidence and optional/partial label probabilities. Score answers contain finite expected score in [0,level_count-1], reported confidence and optional/partial level probabilities. Confidence and probabilities are finite in [0,1]. Extra/missing IDs, wrong answer types and unknown labels/levels are errors. Each BackendBinding, ModelRequest and Answers carries an explicit DistributionPolicy: Strict or Approximate { max_mass_error: Probability }. No legacy/default deserialization or implicit provider policy is supplied. Approximate bounds must be positive and <=0.05; invalid bounds return InvalidValue before inference. This is a host acceptance policy, not a verified provider rounding guarantee. Strict complete distributions sum to one within numerical tolerance1e-6. Approximate complete distributions require positive total mass and absolute error <= configured max_mass_error +1e-6. Partial distributions always have mass <=1+1e-6; missing entries are never filled or normalized. State is Complete (full coverage within1e-6), Approximate (full coverage outside1e-6 but accepted by policy), Partial or Unavailable. Raw answer entries, selected label, confidence, expected score and actual model identity are unchanged. Answers exposes a derived DistributionAdjustment {raw_mass, scale} only for accepted Approximate complete distributions, where scale=1/raw_mass; exact/partial/unavailable distributions have no adjustment. Deserialized Answers revalidate their recorded policy and raw values. Invalid mass diagnostics include question ID, coverage, supplied mass and allowed error; gross error and zero full mass refuse. Actual model identity is non-empty; an expected identity is checked only when the caller configures one.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-005-AC-1 | A valid mixed response preserves expected score, option probabilities, distribution completeness, actual model identity and, when present, the model weights digest. | Test (TC-005) |
| FR-005-AC-2 | Missing/extra IDs, mismatched types, unknown labels, invalid mass and non-finite values return InvalidAnswer or MissingAnswer. | Test (TC-005) |
| FR-005-AC-3 | A partial or unavailable distribution remains distinguishable and never becomes a zero-valued complete distribution. | Test (TC-005) |
| FR-005-AC-4 | Approximate policy accepts complete Choice and Score totals0.99 and1.01 at inclusive configured bounds, retains raw values and exposes mass/scale and Approximate state; Strict, out-of-bound/zero full mass, invalid policy and malformed labels/types/unit values refuse with diagnostic context. Partial excess mass still refuses. | Test (TC-005) |

## Dependencies

- [FR-004](FR-004-declare-typed-questions.md)
