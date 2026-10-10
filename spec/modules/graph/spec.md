---
type: master-requirements
name: sapho-graph
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

This module owns the graph contracts implemented by `sapho-graph`.

## Scope

The functional requirements below define its complete first-release behavior. Domain rules, model training, review rendering and repair loops remain consumers' responsibilities.

## System Overview

Owning crate: `sapho-graph`. Internal prerequisites: core. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-007: Load declarative graph configuration](functional/FR-007-load-declarative-graph-configuration.md)
- [FR-008: Compile checked acyclic graph](functional/FR-008-compile-checked-acyclic-graph.md)
- [FR-009: Type conditional node outputs](functional/FR-009-type-conditional-node-outputs.md)
- [FR-082: Apply a typed calibration map explicitly in a graph](functional/FR-082.md)
- [FR-063: Consume measured roster profiles as typed graph literals](functional/FR-063.md)

## References

See the [workspace specification](../../spec.md) and the functional artifacts owned by this module.
