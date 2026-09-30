---
type: master-requirements
name: sapho-jev
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

This module owns the jev contracts implemented by `sapho-jev`.

## Scope

The functional requirements below define its complete first-release behavior. Domain rules, model training, review rendering and repair loops remain consumers' responsibilities.

## System Overview

Owning crate: `sapho-jev`. Internal prerequisites: core. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-025: Translate System One requests](functional/FR-025-translate-system-one-requests.md)
- [FR-026: Surface bounded model calls](functional/FR-026-surface-bounded-model-calls.md)

## References

See the [workspace specification](../../spec.md) and the functional artifacts owned by this module.
