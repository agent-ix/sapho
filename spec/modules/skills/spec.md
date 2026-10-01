---
type: master-requirements
name: sapho-skills
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Graph creation, tuning and recording workflows using the public CLI.

## Scope

This module owns only the requirements indexed below. Domain rules, model training execution, automatic enforcement hooks and service deployment remain outside its boundary.

## System Overview

Owning component: `plugins/sapho`. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [FR-042: Package graph authoring workflows](functional/FR-042.md)

## References

The [workspace contract](../../spec.md) and the [shared CLI/evidence limits](../cli/non-functional/NFR-005.md).
