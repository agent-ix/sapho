---
type: master-requirements
name: sapho-systemone
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Single request translation owner for Jev and CLM's verified identical System One request contracts.

## Scope

The existing [FR-025](../jev/functional/FR-025-translate-system-one-requests.md) translation and [FR-043](../clm/functional/FR-043.md) request translation use SDK-owned wire types here. No HTTP, credentials, provider response decoding, retry or model policy belongs here.

## System Overview

`sapho-systemone` depends only on core among workspace crates and directly consumes the authoritative TypeSafe SDK types. Jev's existing public build_request export refers to this owner; CLM uses the same implementation before bounded serialization. Core retains no transport/SDK dependency.

## Requirements Architecture

[FR-025](../jev/functional/FR-025-translate-system-one-requests.md) and [FR-043](../clm/functional/FR-043.md) own behavior. This is an internal allocation of existing obligations, not a new user-visible feature.

## References

[Workspace](../../spec.md).
