---
type: master-requirements
name: sapho
org: agent-ix
component_type: rust-lib
implementation_language: rust
standards_alignment: [iso-iec-ieee-29148]
---
# Master Requirements Specification

## Purpose

Sapho executes configurable graphs of native code, typed model questions and logic. Consumer applications own domain concepts and interpretation. This complete first-release specification precedes implementation.

## Scope

### 2.1 In Scope

Typed values and provenance; native Rust extension registration; TOML graph loading and pure compilation; bounded acyclic execution; mapped subgraphs, filtering, pairing, joining and collecting; typed System One questions; explicit batching; crisp and heuristic operators; execution traces; hosted Jev adapter; caller-controlled recording and exact offline replay.

### 2.2 Out of Scope

EARS extractors or questions, test-adequacy rules, code-review findings, PR rendering, repository discovery, edit hooks, repair loops, arbitrary scripts, graph cycles, model training, labelled calibration, expert training, local inference runtimes, CLI or service deployment. Downstream applications implement these using the extension boundaries.

## System Overview

An embedding Rust application supplies inputs, graph config, native primitives, backend bindings and limits. Native primitives are trusted application code. Model responses and config are validated data. The Jev adapter calls a configured SDK client. Recording paths are chosen explicitly by the caller.

## Requirements Architecture

| Module | Owning crate | Responsibility |
|--------|--------------|----------------|
| [core](modules/core/spec.md) | `sapho-core` | Typed values, evidence and extension contracts |
| [graph](modules/graph/spec.md) | `sapho-graph` | Declarative graph definition and compilation |
| [runtime](modules/runtime/spec.md) | `sapho-runtime` | Bounded graph execution and collection identity |
| [logic](modules/logic/spec.md) | `sapho-runtime` | Crisp logic and heuristic degree operations |
| [jev](modules/jev/spec.md) | `sapho-jev` | Hosted Jev backend adapter |
| [recording](modules/recording/spec.md) | `sapho-recording` | Exact recording and offline replay |

## Crate Dependency Boundaries

```mermaid
flowchart TD
 runtime[sapho-runtime] --> graph[sapho-graph]
 runtime --> core[sapho-core]
 graph --> core
 jev[sapho-jev] --> core
 recording[sapho-recording] --> core
```

The root `sapho` package is an embedding facade over these crates; it owns no independent behavior and its optional `jev` feature is disabled by default. The logic specification module shares the runtime crate; logic operators have no transport or EARS dependency. Core owns the shared ports so recording and Jev need no runtime dependency.

## Public Contract

`GraphSpec::parse` loads TOML. `compile` binds checked graph operations and native implementations. `Engine::run` accepts named Datum values and finite RunLimits, returning RunResult or RunFailure with partial Trace. Registries reject duplicate names. ModelBackend is the asynchronous inference seam; Primitive is the synchronous native-code seam.

Each binding names a graph input, a node port, or a typed literal and may select a record-field path. Each operation declares its input/output port types. Guarded ports are Optional; consumers explicitly coalesce them. Model calls accept a Record state and ordered Questions and return validated Answers. Explicit ask nodes define batching; the executor never merges different ask nodes.

## Operation Catalog

| Operation | Inputs | Output | Parameters |
|-----------|--------|--------|------------|
| code | Declared by registered Primitive | Declared by Primitive | Primitive name and typed params |
| questions | None | result: Questions | Ordered definitions |
| ask | state: Record; questions: Questions | answers: Answers; model: Text | Backend name |
| map | items: List(T); captures matching subgraph inputs | result: List(U) | Subgraph with item input and result output |
| filter | items: List(T); mask: List(Boolean) | result: List(T) | None |
| pairs | left: List(L); right: List(R) | result: List(Record(left,right)) | None |
| join | left/right record lists | result: matched pairs | left_key, right_key |
| collect | items: List(List(T)) | result: List(T) | None |
| and/or | a,b: Boolean | result: Boolean | None |
| not | value: Boolean | result: Boolean | None |
| equal/less/less_equal/greater/greater_equal | a,b: compatible scalars | result: Boolean | Comparator |
| probability | answers: Answers | result: Probability | question, labels |
| degree | value: Probability or Number | result: Degree | None |
| min/max | values: List(Degree) | result: Degree | empty degree |
| weighted_mean | values: List(Degree); weights: List(Number) | result: Degree | empty degree |
| complement | value: Degree | result: Degree | None |
| coalesce | value: Optional(T); default: T | result: T | None |

## Execution and Failure Model

Compiled graphs are immutable. Node statuses are completed, skipped or failed. Guard false produces Optional absence without work; failures abort the run and retain the partial trace. Model calls may execute concurrently within a ready stage, but output and trace order remain deterministic. Maps reuse this executor and its global counters. A run has no implicit retry or repair loop. Native code is cooperative and cannot be forcibly preempted after it starts.

Errors use a typed ErrorCode plus contextual fields, not message parsing: Config, DuplicateId, UnknownPrimitive, UnknownBackend, UnknownReference, Cycle, TypeMismatch, MissingInput, InvalidValue, InvalidAnswer, MissingAnswer, UnsupportedDistribution, LimitExceeded, DeadlineExceeded, CodeFailed, BackendFailed, Unauthorized, RateLimited, ServiceValidation, ModelMismatch, ReplayMiss, RecordingIo and RecordingMismatch.

## Probability Semantics

Probability, reported confidence, expected ordinal score and heuristic Degree are different quantities. Degree reductions do not claim joint probability or labelled correctness. Missing probability entries are not zero. Consumer thresholds, domain ambiguity labels and expert-selection policy remain downstream.

## Verification Strategy

Each functional artifact declares three acceptance criteria and one planned TC. Integration artifacts exercise cross-crate seams. Tests bind exact AC IDs using `Trace:` tags. The EARS-shaped synthetic integration test establishes engine composition only, not semantic model quality. Default tests run offline.

## Lifecycle and Change Control

First-release requirements are specified and reviewed before code. Requirement IDs are global across the module directories. Changes update their owning artifact and affected tests. Implemented/verified status is reported from measured evidence rather than a copied ticket claim.

## References

The authoritative TypeSafe SDK is a package dependency. Native domain adapters and private evidence are supplied from their owning repositories, never copied into Sapho.
