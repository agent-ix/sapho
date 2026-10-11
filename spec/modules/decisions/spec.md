---
type: master-requirements
name: sapho-decisions
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

`sapho-decisions` implements the verified OpenAI Decisions API public beta contract as one Sapho `ModelBackend` for finite typed questions.

## Scope

### In Scope

Exact `POST https://api.openai.com/v1/decisions` translation for `gpt-6-luna`, checked text and inline-image context, typed predicate/choice/score responses, per-question refusal, provider usage and model identity, bounded bearer-authenticated transport, and exact offline recording/replay.

### Out of Scope

Responses or Chat Completions substitutions, generated prose or extraction, hosted image retrieval, file IDs, deployment or model installation, model-quality claims, and live acceptance while the account lacks quota.

## System Overview

The crate depends only on `sapho-core` among workspace crates. The host supplies a redacted credential and the fixed official endpoint; CLI binding metadata contains no endpoint or secret. Runtime retains policy-sensitive `Answers` validation. The adapter exposes a native transport seam for offline codec and HTTP tests. Recording remains core-only.

## Requirements Architecture

- [FR-085](functional/FR-085.md): request and answer translation
- [FR-086](functional/FR-086.md): bounded authenticated transport
- [FR-087](../cli/functional/FR-087.md): stock CLI binding and credential preparation
- [IT-018](integration/IT-018.md): offline wire, failure and replay evidence

## References

[Official Decisions guide](https://developers.openai.com/api/docs/guides/decisions), [official create reference](https://developers.openai.com/api/reference/resources/decisions/methods/create), [FR-047](../clm/functional/FR-047.md), [core ModelBackend](../core/functional/FR-006-define-replaceable-model-backend.md), and the [workspace](../../spec.md). Contract inspected 2026-10-10; beta changes require renewed review.
