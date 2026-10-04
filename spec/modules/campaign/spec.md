---
type: master-requirements
name: sapho-campaign
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Execute resumable jobs through native Sapho with inspectable evidence and domain-owned acceptance.

## Scope

### In Scope

Fenced durable storage, stage attempts, explicit backend bindings, guarded controls, recovery, reusable adapter interface, generic graph adapter, CLI and attached dashboard.

### Out of Scope

Domain semantics, dataset eligibility, EARS quotas, private Git publication, model installation, training, dynamic plugins and automatic retries.

## Requirements Architecture

[FR-049](functional/FR-049.md) owns storage. [FR-050](functional/FR-050.md) owns attempts. [FR-051](functional/FR-051.md) owns adapter isolation. [FR-052](functional/FR-052.md) owns commands and dashboard. [IT-007](integration/IT-007.md) covers composition.

## System Overview

sapho-campaign contains focused storage, lifecycle, adapter, execution and control modules. It depends on core/graph/runtime/recording and bounded storage libraries. sapho-campaign-tui depends on the public campaign snapshot/control API and Ratatui. sapho-cli is command wiring over the same services. Optional facade features preserve the existing dependency floor. Trusted compiled adapters can seal domain records in a fenced transaction; this is not a sandbox. EARS and Quire remain downstream.

## References

[Workspace](../../spec.md). Runtime graph execution remains authoritative; no second DAG engine is introduced.
