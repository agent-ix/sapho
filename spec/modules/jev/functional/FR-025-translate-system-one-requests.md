---
id: FR-025
title: "Translate System One requests"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-004"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-005"
    type: implements
  - target: "ix://agent-ix/sapho/StR-005"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-004"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-006"
    type: depends_on
---
# FR-025: Translate System One requests

## Description

The Jev adapter SHALL send the declared state and ordered questions through the authoritative TypeSafe SDK.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Use typesafe-sdk-client/questions/answers/http/config/model dependencies version 0.6.2. The adapter exposes a constructor accepting a configured SDK client, so the host owns auth, endpoint and client settings; SDK types do not escape core/graph/runtime. Translate Record state into plain JSON values, not Sapho tagged Value envelopes. Lists send their values, not item IDs or source sidecars unless the application placed these in state. Preserve ordered choices and requested model. Translate SDK response into core answers without discarding probabilities or expected score.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-025-AC-1 | An injected SDK transport observes plain state, ordered question criteria and the requested model. | Test (TC-025) |
| FR-025-AC-2 | A mixed SDK response is translated without losing model identity or partial probabilities. | Test (TC-025) |
| FR-025-AC-3 | A serialization/type failure makes no transport request. | Test (TC-025) |

## Dependencies

- [FR-004](../../core/functional/FR-004-declare-typed-questions.md)
- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-006](../../core/functional/FR-006-define-replaceable-model-backend.md)
