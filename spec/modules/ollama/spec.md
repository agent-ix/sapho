---
type: master-requirements
name: sapho-ollama
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Provide bounded local Ollama typed question inference as a reusable Sapho backend.

## Scope

### In Scope

Explicit loopback native service protocol, typed answers, self-report disclosure, bounded raw evidence.

### Out of Scope

Service installation, model download/training, domain semantics, probability calibration or automatic retries.

## Requirements Architecture

[FR-048](functional/FR-048.md) owns this boundary. The adapter depends on sapho-core; runtime retains core validation.

## System Overview

A host supplies finite configuration; asynchronous HTTP executes one generation per core request and returns raw typed answers.

## References

[Native generation protocol](https://docs.ollama.com/api/generate), [structured output](https://docs.ollama.com/capabilities/structured-outputs), [workspace](../../spec.md).
