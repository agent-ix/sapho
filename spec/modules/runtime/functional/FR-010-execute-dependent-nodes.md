---
id: FR-010
title: "Execute dependent nodes"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-004"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-005"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-006"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-008"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-009"
    type: depends_on
---
# FR-010: Execute dependent nodes

## Description

The Sapho executor SHALL evaluate each runnable node from the outputs of its declared dependencies.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Engine owns the compiled graph and backend bindings. Run receives named Datum inputs and explicit RunLimits. Validate the whole input set before work. Execute deterministic topological stages; pure/code/logic nodes retain declaration order, and ready ask nodes may run concurrently up to the caller ceiling. Completed result order and trace order remain topological/declaration order, independent of completion timing. Native code runs via bounded spawn_blocking; await its result, convert worker failure to CodeFailed, and expose cooperative deadline observation. A node failure aborts the run with a partial trace; named outputs are returned only for a completed run. No partial successful verdict is fabricated.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-010-AC-1 | A dependent node receives its predecessor values, and independent model completion order does not reorder outputs. | Test (TC-010) |
| FR-010-AC-2 | Invalid input names/types fail before any code or model invocation. | Test (TC-010) |
| FR-010-AC-3 | A native or backend failure returns RunFailure with the partial trace and prevents dependent execution. | Test (TC-010) |

## Dependencies

- [FR-005](../../core/functional/FR-005-validate-model-answers.md)
- [FR-006](../../core/functional/FR-006-define-replaceable-model-backend.md)
- [FR-008](../../graph/functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-009](../../graph/functional/FR-009-type-conditional-node-outputs.md)
