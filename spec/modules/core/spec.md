---
type: master-requirements
name: sapho-core
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

This module owns the core contracts implemented by `sapho-core`.

## Scope

The functional requirements below define its complete first-release behavior. Domain rules, model training, review rendering and repair loops remain consumers' responsibilities.

## System Overview

Owning crate: `sapho-core`. Internal prerequisites: none. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-001: Validate typed values](functional/FR-001-validate-typed-values.md)
- [FR-002: Preserve source attribution](functional/FR-002-preserve-source-attribution.md)
- [FR-003: Register custom Rust primitives](functional/FR-003-register-custom-rust-primitives.md)
- [FR-004: Declare typed questions](functional/FR-004-declare-typed-questions.md)
- [FR-005: Validate model answers](functional/FR-005-validate-model-answers.md)
- [FR-006: Define replaceable model backend](functional/FR-006-define-replaceable-model-backend.md)

- [FR-032: Decode plain data by a declared type](functional/FR-032.md)
- [FR-048: Define a one-call structured extraction port](functional/FR-048.md)
- [FR-081: Represent calibrated probability and a checked calibration map](functional/FR-081.md)

## References

See the [workspace specification](../../spec.md) and the functional artifacts owned by this module.
