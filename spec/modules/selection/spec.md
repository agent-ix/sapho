---
type: master-requirements
name: sapho-selection
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Bounded acquisition of identified file, Git-diff and JSON units outside the executor.

## Scope

This module owns only the requirements indexed below. Domain rules, model training execution, automatic enforcement hooks and service deployment remain outside its boundary.

## System Overview

Owning component: `sapho-select`. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-039: Select bounded attributable files](functional/FR-039.md)
- [FR-040: Select explicit Git changes](functional/FR-040.md)
- [FR-041: Select typed values from structured data](functional/FR-041.md)

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
