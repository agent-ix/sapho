---
type: master-requirements
name: sapho-ollama
org: agent-ix
component_type: rust-lib
implementation_language: rust
---
# Master Requirements Specification

## Purpose

`sapho-ollama` calls an already running Ollama server to extract one structured, schema-checked record per request, to answer typed questions with probabilities read from the answer tokens, and to embed text. Each request stands alone: fixed instructions, one input and one JSON Schema, with nothing carried over from earlier calls. Prompt size is counted in tokens, so an input that does not fit the model's context is reported as too large instead of being cut or failing the caller's run.

## Scope

### In Scope

The [Extractor](../core/functional/FR-048.md) and ModelBackend implementations over Ollama's generate endpoint, the latter deriving answer probabilities from answer-token log-probabilities; explicit generation settings per binding; a lower-bound token estimate before a call, server-side refusal of prompts longer than the context, and the reported token count after it; a `TooLarge` outcome; one request in flight per process; model identity, digest, usage and raw exchange bytes on every response; embeddings over Ollama's embed endpoint; host-configured endpoint, timeout and byte ceilings.

### Out of Scope

Installing, starting, stopping or deploying Ollama, and downloading or creating models; streaming responses; chat requests, conversation context and any state carried between calls; concurrent or batched GPU scheduling; retries and redirects; model accuracy claims; domain prompts, schemas and vocabularies, which belong to consumers.

## System Overview

Owning crate: `sapho-ollama`, which depends only on `sapho-core` among workspace crates. The host supplies a binding per model and task setting (endpoint, model, thinking on or off, context size, output limit, maximum bytes per token, timeout and ceilings). The adapter performs bounded asynchronous HTTP; schema checking of extracted values is done by the core port, and answer validation of typed questions by core answer validation. See the [workspace boundaries](../../spec.md).

## Requirements Architecture

- [StR-012: Label and extract with a local model server](stakeholder/StR-012.md)
- [US-012: Extract one structured record per item with a local model](usecase/US-012.md)
- [FR-049: Send one stateless generate request per extraction](functional/FR-049.md)
- [FR-050: Count prompt size in tokens and report inputs that do not fit](functional/FR-050.md)
- [FR-051: Serialize and bound Ollama HTTP calls](functional/FR-051.md)
- [FR-052: Record model identity, usage and raw bodies](functional/FR-052.md)
- [FR-053: Embed text through the same serialized client](functional/FR-053.md)
- [FR-054: Answer typed questions from answer-token log-probabilities](functional/FR-054.md)
- [IT-007: Extract through a loopback Ollama server](integration/IT-007.md)

## References

The [Extractor port](../core/functional/FR-048.md), the [crate boundaries](../core/non-functional/NFR-001.md), and Ollama's public API documentation for `/api/generate`, `/api/embed` and `/api/show`; behaviour noted as measured was observed on Ollama 0.32.14 with `qwen3:30b`.
