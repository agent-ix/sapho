---
id: FR-028
title: "Replay exact recorded requests"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-004"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/US-006"
    type: implements
  - target: "ix://agent-ix/sapho/StR-006"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-006"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-027"
    type: depends_on
---
# FR-028: Replay exact recorded requests

## Description

The replay backend SHALL return a recorded response only for an exact request and backend identity match.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

The exact request includes DistributionPolicy. Raw ModelResponse remains unchanged; loading/exporting/retention use that request policy through core validation. Projection adjustments are derived, never written over raw values. A request with a different policy is a different replay key. No compatibility reader for older recordings is supplied.


ReplayBackend loads typed recordings with an explicit byte ceiling. Exact matching includes state, question text, question IDs, choice order, requested model and backend binding identity; no digest-only equality is required. Validate each recorded answer against its request before accepting the recording. Repeated identical exchanges with equal responses deduplicate; conflicting responses for the same exact request are RecordingMismatch, rather than first/last wins. Replay misses return ReplayMiss and have no live delegate, key lookup or network path. An optional expected model applies identically to live and replay responses.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-028-AC-1 | Exact requests replay the complete response without transport work. | Test (TC-028) |
| FR-028-AC-2 | Changed choice order, state, model, instructions or backend identity causes ReplayMiss. | Test (TC-028) |
| FR-028-AC-3 | Conflicting duplicate records and malformed distributions refuse loading. | Test (TC-028) |
| FR-028-AC-4 | Approximate-policy recording export/load/replay retains exact raw0.99/1.01 values and reproduces adjustment/projection; changing only the policy causes ReplayMiss. | Test (TC-028) |

## Dependencies

- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-006](../../core/functional/FR-006-define-replaceable-model-backend.md)
- [FR-027](FR-027-record-successful-backend-exchanges.md)
