---
id: FR-011
title: "Enforce run resource limits"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-002"
    type: "references"
  - target: "ix://agent-ix/sapho/US-003"
    type: implements
  - target: "ix://agent-ix/sapho/StR-003"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-010"
    type: depends_on
---
# FR-011: Enforce run resource limits

## Description

The Sapho executor SHALL stop scheduling work when a caller-supplied run limit is exhausted.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

RunLimits requires non-zero ceilings for executed node instances, expanded collection items, model requests, concurrent model/native work, serialized data bytes and elapsed duration. Time is measured with Instant. Counts apply cumulatively across subgraphs; input data bytes and each produced payload are accounted cumulatively before scheduling dependents. Serialized bytes are measured with a bounded writer rather than allocating an unbounded temporary JSON buffer. Native-code output allocations inside the trusted primitive itself are outside this guarantee. An operation checks projected expansion with checked arithmetic before allocation. Deadline wraps asynchronous model/native waits; native work already running cannot be forcibly terminated and must cooperate using PrimitiveContext. No retry or repair loop exists in runtime. Cancellation/deadline returns a partial trace and drops pending model futures.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-011-AC-1 | An expansion exactly at its ceiling succeeds; one exceeding it fails before allocating the expanded collection. | Test (TC-011) |
| FR-011-AC-2 | Subgraph work counts toward the same run counters as its parent. | Test (TC-011) |
| FR-011-AC-3 | A pending model future is cancelled at deadline and the failure retains preceding completed nodes. | Test (TC-011) |

## Dependencies

- [FR-010](FR-010-execute-dependent-nodes.md)
