---
type: master-requirements
name: sapho-evidence
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Curated labelled cases, reproducible measurements, development tuning and training exports.

## Scope

This module owns only the requirements indexed below. Domain rules, model training execution, automatic enforcement hooks and service deployment remain outside its boundary.

## System Overview

Owning component: `sapho-evidence`. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-036: Measure declared outputs against labelled cases](functional/FR-036.md)
- [FR-037: Compare bounded development candidates](functional/FR-037.md)
- [FR-038: Export curated training cases](functional/FR-038.md)

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
