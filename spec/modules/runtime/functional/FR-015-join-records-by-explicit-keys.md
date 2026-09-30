---
id: FR-015
title: "Join records by explicit keys"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-011"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-014"
    type: depends_on
---
# FR-015: Join records by explicit keys

## Description

The Sapho join operation SHALL produce ordered matching pairs for equal declared key values.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Join accepts record lists and explicit left_key/right_key field names. Keys must be scalar Text, Boolean or finite Number of the same type. It is an inner many-to-many join: enumerate left items in order, then matching right items in right order. Preserve both values and source references in left/right records with collision-safe pair IDs. Missing keys and wrong key types fail. Unmatched items produce no output; duplicate keys deliberately produce all matching pairs. Enforce projected output cardinality before materializing results.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-015-AC-1 | Repeated keys produce all matching pairs in stable order; unmatched records disappear. | Test (TC-015) |
| FR-015-AC-2 | Missing or mismatched key fields fail compilation or input validation. | Test (TC-015) |
| FR-015-AC-3 | A large many-to-many match fails its expansion budget before materializing the join. | Test (TC-015) |

## Dependencies

- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-010](FR-010-execute-dependent-nodes.md)
- [FR-011](FR-011-enforce-run-resource-limits.md)
- [FR-014](FR-014-form-bounded-candidate-pairs.md)
