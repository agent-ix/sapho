---
id: FR-016
title: "Collect nested identified results"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-011"
    type: depends_on
---
# FR-016: Collect nested identified results

## Description

The Sapho collect operation SHALL flatten one level of identified result collections.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Input is List(List(T)); concatenate inner collections in outer then inner order. Preserve inner IDs, values and source references. Duplicate resulting ItemIds are rejected rather than silently dropped or renamed. Empty inner lists contribute nothing; an empty outer list returns empty. Expansion and serialized output limits apply.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-016-AC-1 | Nested results flatten in deterministic order without altering their identities. | Test (TC-016) |
| FR-016-AC-2 | Duplicate resulting IDs return DuplicateId. | Test (TC-016) |
| FR-016-AC-3 | Empty collections produce an empty result without model work. | Test (TC-016) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
- [FR-011](FR-011-enforce-run-resource-limits.md)
