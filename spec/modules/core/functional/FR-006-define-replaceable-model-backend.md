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

A ModelResponse SHALL carry an optional raw exchange, the request body sent and the response body received, using the RawExchange type of extraction ([FR-048](FR-048.md)): body text only, never HTTP headers, URLs or credentials. It is omitted from JSON when absent, so recordings made without it still load. Recordings and traces keep it ([FR-027](../../recording/functional/FR-027-record-successful-backend-exchanges.md), [FR-017](../../runtime/functional/FR-017-produce-execution-evidence.md)), so a probability can be audited against the bytes that produced it after replay.

When a backend that retains the raw exchange fails after an exchange took place, the SaphoError SHALL keep that raw exchange and the reported usage in typed fields; the conversion of an ExtractError into a SaphoError carries both. The bytes travel only in those fields: the error's message, its Display text, log output and the CLI's error output ([FR-046](../../cli/functional/FR-046.md)) never contain them.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-006-AC-1 | A host-defined backend executes through the same request/response types as a supplied adapter. | Test (TC-006) |
| FR-006-AC-2 | Duplicate backend registration is rejected without replacement. | Test (TC-006) |
| FR-006-AC-3 | Backend failure retains a stable error code and produces no answer value. | Test (TC-006) |
| FR-006-AC-4 | A ModelResponse with a raw exchange round-trips through JSON with its bytes unchanged; one without it serializes with no `raw` member, and a recording written without the member loads. | Test (TC-006) |
| FR-006-AC-5 | An ExtractError carrying a raw exchange and usage, converted into a SaphoError, still exposes both unchanged in typed fields, while the error's message and Display text contain no sentinel string planted in either body. | Test (TC-006) |
| FR-006-AC-6 | A backend whose transport sends a sentinel credential in an HTTP header produces a ModelResponse whose raw exchange, serialized, contains neither the sentinel nor any header name. | Test (TC-006) |

## Dependencies

- [FR-004](FR-004-declare-typed-questions.md)
- [FR-005](FR-005-validate-model-answers.md)
