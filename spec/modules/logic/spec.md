---
type: master-requirements
name: sapho-logic
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

This module owns the logic contracts implemented by `sapho-runtime`.

## Scope

The functional requirements below define its complete first-release behavior. Domain rules, model training, review rendering and repair loops remain consumers' responsibilities.

## System Overview

Owning crate: `sapho-runtime`. Internal prerequisites: core, graph. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-018: Evaluate crisp Boolean operations](functional/FR-018-evaluate-crisp-boolean-operations.md)
- [FR-019: Compare facts explicitly](functional/FR-019-compare-facts-explicitly.md)
- [FR-020: Project model probability mass](functional/FR-020-project-model-probability-mass.md)
- [FR-021: Convert evidence to heuristic degree](functional/FR-021-convert-evidence-to-heuristic-degree.md)
- [FR-022: Combine heuristic degrees](functional/FR-022-combine-heuristic-degrees.md)
- [FR-023: Complement a heuristic degree](functional/FR-023-complement-a-heuristic-degree.md)
- [FR-024: Coalesce explicit optional values](functional/FR-024-coalesce-explicit-optional-values.md)
- [FR-077: Merge exactly one present guarded value](functional/FR-077.md)

## References

See the [workspace specification](../../spec.md) and the functional artifacts owned by this module.
