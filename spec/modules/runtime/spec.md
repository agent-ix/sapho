---
type: master-requirements
name: sapho-runtime
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

This module owns the runtime contracts implemented by `sapho-runtime`.

## Scope

The functional requirements below define its complete first-release behavior. Domain rules, model training, review rendering and repair loops remain consumers' responsibilities.

## System Overview

Owning crate: `sapho-runtime`. Internal prerequisites: core, graph. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-010: Execute dependent nodes](functional/FR-010-execute-dependent-nodes.md)
- [FR-011: Enforce run resource limits](functional/FR-011-enforce-run-resource-limits.md)
- [FR-012: Map reusable subgraphs](functional/FR-012-map-reusable-subgraphs.md)
- [FR-013: Filter identified collections](functional/FR-013-filter-identified-collections.md)
- [FR-014: Form bounded candidate pairs](functional/FR-014-form-bounded-candidate-pairs.md)
- [FR-015: Join records by explicit keys](functional/FR-015-join-records-by-explicit-keys.md)
- [FR-016: Collect nested identified results](functional/FR-016-collect-nested-identified-results.md)
- [FR-017: Produce execution evidence](functional/FR-017-produce-execution-evidence.md)

## References

See the [workspace specification](../../spec.md) and the functional artifacts owned by this module.
