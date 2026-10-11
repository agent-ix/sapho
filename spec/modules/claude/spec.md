---
type: master-requirements
name: sapho-claude
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

`sapho-claude` supplies one host-configured Claude Messages `ModelBackend` for independent typed Ask readings.

## Scope

### In Scope

One structured-output Messages call per core request; Boolean, Choice and Score answer validation; actual model and usage; bounded authenticated transport; stock CLI binding; exact offline recording and replay.

### Out of Scope

EARS annotation and review-queue policy, Dataset changes, image input, tool execution, streaming, extraction, model-quality claims and live-account acceptance without access.

## System Overview

The adapter depends only on `sapho-core` among workspace crates. Host configuration supplies the endpoint, model binding and redacted credential; the graph only selects a backend binding. The runtime validates core answers. Recording retains exact core requests/responses and bounded raw HTTP bodies, never authentication headers.

## Requirements Architecture

- [FR-088](functional/FR-088.md): typed Messages codec
- [FR-089](functional/FR-089.md): bounded authenticated transport
- [FR-090](../cli/functional/FR-090.md): CLI preparation and offline replay
- [IT-019](integration/IT-019.md): synthetic wire, error and replay evidence

## References

[Messages create reference](https://platform.claude.com/docs/en/api/messages/create), [structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs), [authentication](https://platform.claude.com/docs/en/manage-claude/authentication), [core backend](../core/functional/FR-006-define-replaceable-model-backend.md), and [workspace](../../spec.md). Official contract inspected 2026-10-10; provider changes require renewed review.
