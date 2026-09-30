---
id: FR-026
title: "Surface bounded model calls"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-005"
    type: implements
  - target: "ix://agent-ix/sapho/StR-005"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-006"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-011"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-025"
    type: depends_on
---
# FR-026: Surface bounded model calls

## Description

The Jev adapter SHALL return transport and service failures through structured backend errors.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

The host configures SDK credentials and endpoint outside graph config. Adapter construction disables SDK retries; one backend invocation produces at most one transport attempt. Runtime owns deadline and model-call count, while host opt-in model tests require explicit credentials. Unauthorized, rate limited, validation and network failures remain distinguishable codes. The adapter has no silent model fallback, endpoint switching or learned routing; actual identity is reported and optional expected identity is enforced by core validation.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-026-AC-1 | 401, 422, 429 and connection errors are returned under distinct structured error codes. | Test (TC-026) |
| FR-026-AC-2 | A permanently rate-limited response causes one attempt and no invented answer. | Test (TC-026) |
| FR-026-AC-3 | A configured expected-model mismatch fails with the actual identity retained in evidence. | Test (TC-026) |

## Dependencies

- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-006](../../core/functional/FR-006-define-replaceable-model-backend.md)
- [FR-011](../../runtime/functional/FR-011-enforce-run-resource-limits.md)
- [FR-025](FR-025-translate-system-one-requests.md)
