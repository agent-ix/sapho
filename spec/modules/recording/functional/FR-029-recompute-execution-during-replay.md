---
id: FR-029
title: "Recompute execution during replay"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-006"
    type: implements
  - target: "ix://agent-ix/sapho/StR-006"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-017"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-028"
    type: depends_on
---
# FR-029: Recompute execution during replay

## Description

The Sapho replay integration SHALL reevaluate code and logic nodes using recorded model exchanges.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

A replay-backed Engine is the same executor as a live-backed Engine. Only its ModelBackend binding changes. It reconstructs requests through the current graph and native code, matches them exactly, then runs the ordinary answer validation and downstream operators. Changed code or graph behavior that changes a request misses; changed downstream logic recomputes and can change outputs. No saved final-output substitution is available.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-029-AC-1 | A live/scripted recording and replay produce the same outputs and deterministic trace content. | Test (TC-029) |
| FR-029-AC-2 | Changing a downstream threshold changes replayed output while leaving recorded model answers unchanged. | Test (TC-029) |
| FR-029-AC-3 | Changing a code-generated question causes ReplayMiss without inference. | Test (TC-029) |

## Dependencies

- [FR-010](../../runtime/functional/FR-010-execute-dependent-nodes.md)
- [FR-017](../../runtime/functional/FR-017-produce-execution-evidence.md)
- [FR-028](FR-028-replay-exact-recorded-requests.md)
