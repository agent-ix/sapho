---
id: FR-003
title: "Register custom Rust primitives"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-002"
    type: depends_on
---
# FR-003: Register custom Rust primitives

## Description

The Sapho primitive registry SHALL bind each registered name to one Rust implementation and a declared port signature.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

The Primitive trait declares input/output ValueTypes and synchronously evaluates Inputs plus typed parameter values under a PrimitiveContext. The registry rejects duplicate names. Compilation snapshots the signature and binds the same implementation; runtime validates against that captured signature rather than querying a mutable signature again. Core owns these traits and types, with no graph, runtime, transport or filesystem dependency. The context exposes cancellation/deadline observation for cooperative native code. Parameters are application-owned values; the engine validates their values but the primitive owns their domain validation. Primitive errors are returned, not fabricated successful outputs.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-003-AC-1 | A registered transformation receives its declared inputs and parameters and returns a checked output. | Test (TC-003) |
| FR-003-AC-2 | Duplicate registration is rejected as DuplicateId without replacing the first implementation. | Test (TC-003) |
| FR-003-AC-3 | Wrong output types, missing/extra ports and primitive failures are reported before a dependent node runs. | Test (TC-003) |

## Dependencies

- [FR-001](FR-001-validate-typed-values.md)
- [FR-002](FR-002-preserve-source-attribution.md)
