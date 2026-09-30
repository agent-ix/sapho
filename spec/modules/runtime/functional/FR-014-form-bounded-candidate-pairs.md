---
id: FR-014
title: "Form bounded candidate pairs"
type: FR
relationships:
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
  - target: "ix://agent-ix/sapho/FR-011"
    type: depends_on
---
# FR-014: Form bounded candidate pairs

## Description

The Sapho pairs operation SHALL enumerate ordered pairs of identified left and right items.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Inputs left: List(L) and right: List(R) produce List(Record{left:L,right:R}). Enumerate left order then right order, including self-pairs when the supplied sets overlap; applications can filter them explicitly. Pair ItemId is an unambiguous serialization of the two input IDs, never a delimiter-concatenation collision. Sources are the union of the two focal items. Empty either side yields empty. Check Cartesian cardinality and cumulative limits before allocation; no text-based merging or automatic truncation.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-014-AC-1 | Two left and three right items produce exactly six ordered pairs and distinct pair IDs. | Test (TC-014) |
| FR-014-AC-2 | IDs containing separator characters cannot collide across distinct pairs. | Test (TC-014) |
| FR-014-AC-3 | An over-limit Cartesian product is rejected before output allocation. | Test (TC-014) |

## Dependencies

- [FR-002](../../core/functional/FR-002-preserve-source-attribution.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
- [FR-011](FR-011-enforce-run-resource-limits.md)
