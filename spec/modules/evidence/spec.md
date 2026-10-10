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

Owning component: `sapho-evidence` for dataset validation, scoring, ranking and export-record construction, with only a core dependency. The `sapho-cli` host reads paths, loads/compiles/runs candidates, invokes the pure calculations and writes artifacts; those orchestration responsibilities never introduce filesystem or runtime dependencies into the evidence crate. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-036: Measure declared outputs against labelled cases](functional/FR-036.md)
- [FR-037: Compare bounded development candidates](functional/FR-037.md)
- [FR-038: Export curated training cases](functional/FR-038.md)
- [FR-067: Report labelled metrics by declared slice and window](functional/FR-067.md)
- [FR-068: Compare confidence distributions without labels](functional/FR-068.md)
- [IT-011: Verify slice metrics and recording confidence drift](integration/IT-011.md)

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
