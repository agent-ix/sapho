---
id: FR-009
title: "Type conditional node outputs"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-002"
    type: implements
  - target: "ix://agent-ix/sapho/StR-002"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
---
# FR-009: Type conditional node outputs

## Description

The Sapho compiler SHALL expose guarded node outputs as Optional values.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

A guard binding must be Boolean. Each guarded output type is Optional(original type). When true, runtime wraps the result in Some; when false it emits None for each declared port. Consumers must accept Optional or use the coalesce operator. No implicit unwrapping, truth conversion or default value is inserted. Guards participate in dependency checks. Unselected nodes are recorded as skipped, not failed.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-009-AC-1 | A guarded output cannot connect to a mandatory port without explicit coalescing. | Test (TC-009) |
| FR-009-AC-2 | A false guard emits typed absence and performs no primitive/model work. | Test (TC-009) |
| FR-009-AC-3 | A true guard wraps the correctly typed result and makes it available downstream. | Test (TC-009) |

## Dependencies

- [FR-008](FR-008-compile-checked-acyclic-graph.md)
