---
type: master-requirements
name: sapho-cli
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Checked command-line invocation and host-owned I/O, backends and exit policy.

## Scope

This module owns only the requirements indexed below. Domain rules, model training execution, automatic enforcement hooks and service deployment remain outside its boundary.

## System Overview

Owning component: `sapho-cli`. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-033: Inspect a graph without executing it](functional/FR-033.md)
- [FR-034: Invoke bounded graphs from data](functional/FR-034.md)
- [FR-035: Record and replay CLI evaluations](functional/FR-035.md)

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
