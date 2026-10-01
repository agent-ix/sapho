---
id: FR-027
title: "Record successful backend exchanges"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-006"
    type: implements
  - target: "ix://agent-ix/sapho/StR-006"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-004"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-006"
    type: depends_on
---
# FR-027: Record successful backend exchanges

## Description

The recording backend SHALL retain each successful request and response as an explicit typed exchange.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

The exact request includes DistributionPolicy. Export/load retains exact representable f64 values, including values requiring more than two decimal places. Raw ModelResponse remains unchanged; loading/exporting/retention use that request policy through core validation. Projection adjustments are derived, never written over raw values. A request with a different policy is a different replay key. No compatibility reader for older recordings is supplied.


RecordingBackend decorates any ModelBackend. Record exact core request, raw core response (including distributions and actual model), and backend binding identity. Recording has no automatic filesystem path. In-memory exchanges are exportable as a typed JSON recording; a caller can explicitly write to a new path using create_new. File existence/I/O failures are errors, not overwrite permission. Failed backend calls are not stored as successful exchanges; runtime traces retain their failure evidence. Concurrent completion may occur out of order; replay matching never depends on file order. Before exporting, validate request/response and impose a caller-supplied maximum serialized byte count.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-027-AC-1 | A successful decorated call retains an exact request/response exchange. | Test (TC-027) |
| FR-027-AC-2 | An errored call adds no successful exchange. | Test (TC-027) |
| FR-027-AC-3 | Export over the byte limit or to an existing path fails without overwriting it. | Test (TC-027) |

## Dependencies

- [FR-004](../../core/functional/FR-004-declare-typed-questions.md)
- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-006](../../core/functional/FR-006-define-replaceable-model-backend.md)
