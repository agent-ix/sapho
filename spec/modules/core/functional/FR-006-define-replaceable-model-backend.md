---
id: FR-006
title: "Define replaceable model backend"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-004"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
---
# FR-006: Define replaceable model backend

## Description

The Sapho backend registry SHALL resolve a configured backend name to one asynchronous ModelBackend implementation.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

ModelBackend accepts ModelRequest and returns ModelResponse or a structured SaphoError. Registrations reject duplicates. Every ModelRequest includes its BackendId, requested model, optional expected model, Record state and ordered questions. Each binding owns requested model and optional expected model identity; credentials are held by the adapter, never by graph or recording types. The caller can implement another backend without depending on graph or runtime crates. Cancellation drops the model future; usage is optional, not invented.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-006-AC-1 | A host-defined backend executes through the same request/response types as a supplied adapter. | Test (TC-006) |
| FR-006-AC-2 | Duplicate backend registration is rejected without replacement. | Test (TC-006) |
| FR-006-AC-3 | Backend failure retains a stable error code and produces no answer value. | Test (TC-006) |

## Dependencies

- [FR-004](FR-004-declare-typed-questions.md)
- [FR-005](FR-005-validate-model-answers.md)
