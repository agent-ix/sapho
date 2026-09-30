---
id: FR-013
title: "Filter identified collections"
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
# FR-013: Filter identified collections

## Description

The Sapho filter operation SHALL retain only items whose identified Boolean masks are true.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Inputs are items: List(T) and mask: List(Boolean). Require exactly one mask for each item ID, no extra or duplicate masks; mask order need not equal item order. Output retains original item order, IDs, values and sources. Absent or wrong masks are errors, not false. Empty/empty returns empty. Parameters are empty.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-013-AC-1 | A reordered mask selects the correct original items by ID. | Test (TC-013) |
| FR-013-AC-2 | Missing, extra, duplicate or non-Boolean masks are rejected. | Test (TC-013) |
| FR-013-AC-3 | Filtering preserves equal-text items when their individual masks are true. | Test (TC-013) |

## Dependencies

- [FR-001](../../core/functional/FR-001-validate-typed-values.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
- [FR-011](FR-011-enforce-run-resource-limits.md)
