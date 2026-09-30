---
id: FR-008
title: "Compile checked acyclic graph"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-004"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-002"
    type: implements
  - target: "ix://agent-ix/sapho/StR-002"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-003"
    type: depends_on
  - target: "ix://agent-ix/sapho/FR-007"
    type: depends_on
---
# FR-008: Compile checked acyclic graph

## Description

The Sapho compiler SHALL reject a graph whose connections or dependencies cannot be executed according to declared port types.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Compile is pure: no code primitive evaluation, model call, filesystem or process launch. Resolve primitive signatures, input paths, output paths, guards and subgraph references, then compute a stable topological node order using declaration order as the tie-break. Duplicate IDs, unknown primitives/references, wrong/missing/extra ports, non-Boolean guards, cycles and missing output bindings are errors. Compilation caps expanded graph nodes at 4096 and subgraph nesting at 16, checks subgraph-reference cycles, and validates every graph body, including unused definitions. Named outputs carry inferred ValueTypes. A compiled code node binds the registered Arc implementation that supplied its signature, avoiding a separate runtime registry drift.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-008-AC-1 | Reordered forward node references compile into the same valid dependency order without inference. | Test (TC-008) |
| FR-008-AC-2 | Cycles, duplicate IDs, absent references, incompatible types and recursive subgraph definitions fail compilation. | Test (TC-008) |
| FR-008-AC-3 | Compilation never invokes registered code or a model backend and refuses its node/depth ceilings. | Test (TC-008) |

## Dependencies

- [FR-003](../../core/functional/FR-003-register-custom-rust-primitives.md)
- [FR-007](FR-007-load-declarative-graph-configuration.md)
