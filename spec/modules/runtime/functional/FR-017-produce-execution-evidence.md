---
id: FR-017
title: "Produce execution evidence"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-002"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-017: Produce execution evidence

## Description

The Sapho executor SHALL return an ordered trace for all completed, skipped or failed node instances.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Trace entry includes execution path (graph/subgraph/node/item IDs), fully scoped dependency paths, resolved guard when configured, resolved operation inputs, outputs, source references, operation and params, status and optional structured error. Ask evidence includes exact request, raw normalized response, actual model and optional usage. Failure carries prior entries and a failed entry when the node began. Timing is not part of deterministic trace content. Recording is separate: traces are in-memory returned values, not automatically written to disk. Credentials are absent from trace types.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-017-AC-1 | A threshold trace identifies the contributing answers, applied cutoff and source spans. | Test (TC-017) |
| FR-017-AC-2 | A false guard produces a skipped entry with no request evidence. | Test (TC-017) |
| FR-017-AC-3 | A malformed backend answer produces a failed entry retaining request/response evidence. | Test (TC-017) |

## Dependencies

- [FR-002](../../core/functional/FR-002-preserve-source-attribution.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
