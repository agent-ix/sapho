---
type: master-requirements
name: sapho-clm
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

Host-configured CLM System One inference for embedding applications and the CLI.

## Scope

### In Scope

Typed request translation, bounded HTTP transport, validated wire decoding, raw provider evidence and offline capture/replay.

### Out of Scope

Model downloads, installation, training, deployment, rank endpoint, temperature tuning, provider accuracy claims and domain questions.

## System Overview

`sapho-clm` depends on core and the shared systemone request codec among workspace crates. The host supplies an endpoint and optional redacted credential. Runtime retains its existing semantic validation responsibility. CLI resolves credentials synchronously; the adapter performs bounded asynchronous HTTP. Recording depends only on core.

## Requirements Architecture

- [US-011](usecase/US-011.md)
- [StR-011](stakeholder/StR-011.md)
- [FR-043](functional/FR-043.md): CLM request/response translation
- [FR-044](functional/FR-044.md): bounded host-configured HTTP
- [FR-047](functional/FR-047.md): verified official Decisions contract assessment, allocated to the separate [Decisions adapter](../decisions/spec.md)
- [IT-006](integration/IT-006.md): transport/capture/replay

## References

[CLM source](https://github.com/Contrastive-LM/CLM), [reference model](https://huggingface.co/Contrastive-LM/CLM-v0.1-8B), and [workspace](../../spec.md).
