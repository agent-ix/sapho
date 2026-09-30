---
id: FR-024
title: "Coalesce explicit optional values"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-004"
    type: implements
  - target: "ix://agent-ix/sapho/StR-004"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-009"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-024: Coalesce explicit optional values

## Description

The Sapho coalesce evaluator SHALL replace an absent Optional value with the explicitly supplied default.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Inputs value: Optional(T), default: T; output result: T. Some(v) yields v even if v is false, zero, empty text or an empty collection. None yields default. The output references include both resolved inputs for explanation; no undeclared fallback or compatibility reader exists.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-024-AC-1 | Some(false), Some(0) and Some(empty list) remain their original values. | Test (TC-024) |
| FR-024-AC-2 | None yields a default of the same declared type. | Test (TC-024) |
| FR-024-AC-3 | An incompatible default is rejected before execution. | Test (TC-024) |

## Dependencies

- [FR-009](../../graph/functional/FR-009-type-conditional-node-outputs.md)
- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
