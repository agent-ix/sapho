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

The host binding contains an explicit DistributionPolicy. Registration validates its bound before accepting the backend. Runtime copies the policy into ModelRequest and validated Answers; it is never sent as a provider SDK field.


ModelBackend accepts ModelRequest and returns ModelResponse or a structured SaphoError. Registrations reject duplicates. Every ModelRequest includes its BackendId, requested model, optional expected model, Record state and ordered questions. Each binding owns requested model and optional expected model identity; credentials are held by the adapter, never by graph or recording types. The caller can implement another backend without depending on graph or runtime crates. Cancellation drops the model future; usage is optional, not invented.

A ModelResponse SHALL carry an optional raw exchange, the exact request bytes sent and response bytes received, using the same RawExchange type as extraction ([FR-048](FR-048.md)); it is omitted from JSON when absent, so recordings made without it still load. When a backend failure happens after an exchange took place, the SaphoError SHALL keep that raw exchange and the reported usage; the conversion of an ExtractError into a SaphoError carries both. A probability or a failure can then be audited against the bytes that produced it.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-006-AC-1 | A host-defined backend executes through the same request/response types as a supplied adapter. | Test (TC-006) |
| FR-006-AC-2 | Duplicate backend registration is rejected without replacement. | Test (TC-006) |
| FR-006-AC-3 | Backend failure retains a stable error code and produces no answer value. | Test (TC-006) |
| FR-006-AC-4 | A ModelResponse with a raw exchange round-trips through JSON with its bytes unchanged; one without it serializes with no `raw` member, and a recording written without the member loads. | Test (TC-006) |
| FR-006-AC-5 | An ExtractError carrying a raw exchange and usage, converted into a SaphoError, still exposes both unchanged. | Test (TC-006) |

## Dependencies

- [FR-004](FR-004-declare-typed-questions.md)
- [FR-005](FR-005-validate-model-answers.md)
