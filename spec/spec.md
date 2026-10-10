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

Typed values and provenance; native Rust extension registration; YAML/JSON graph loading and pure compilation; bounded acyclic execution; mapped subgraphs, filtering, pairing, joining and collecting; typed System One questions; explicit batching; crisp and heuristic operators; execution traces; hosted Jev and host-configured CLM adapters; the Extractor port for one-call schema-checked structured extraction; a host-configured Ollama backend that calls an already running local inference server for extraction, typed questions answered with probabilities from answer-token log-probabilities, and embeddings, counting prompt size in tokens; caller-controlled recording and exact offline replay.

### 2.2 Out of Scope

EARS extractors or questions, test-adequacy rules, code-review findings, PR rendering, implicit repository discovery, edit hooks, repair loops, arbitrary scripts, graph cycles, model training, expert/model training execution, installing, running or deploying an inference server or service (a backend may call one that is already running), and dataset collection or labelling pipelines. Downstream applications implement these using the extension boundaries.

## System Overview

A Rust embedding application or the Sapho CLI host supplies inputs, graph config, native primitives, backend bindings and limits. Native primitives are trusted application code. Model responses and config are validated data. Jev uses a configured SDK client; CLM uses a bounded host-configured HTTP transport. Recording paths are chosen explicitly by the caller.

## Requirements Architecture

| Module | Owning crate | Responsibility |
|--------|--------------|----------------|
| [core](modules/core/spec.md) | `sapho-core` | Typed values, evidence and extension contracts, including the ModelBackend and Extractor ports |
| [graph](modules/graph/spec.md) | `sapho-graph` | Declarative graph definition and compilation |
| [runtime](modules/runtime/spec.md) | `sapho-runtime` | Bounded graph execution and collection identity |
| [logic](modules/logic/spec.md) | `sapho-runtime` | Crisp logic and heuristic degree operations |
| [systemone](modules/systemone/spec.md) | `sapho-systemone` | Shared source-free request translation into SDK-owned wire types |
| [clm](modules/clm/spec.md) | `sapho-clm` | Host-configured bounded CLM backend adapter |
| [jev](modules/jev/spec.md) | `sapho-jev` | Hosted Jev backend adapter |
| [recording](modules/recording/spec.md) | `sapho-recording` | Exact recording and offline replay |
| [ollama](modules/ollama/spec.md) | `sapho-ollama` | Host-configured Ollama extraction, typed-question and embedding adapter |

| [cli](modules/cli/spec.md) | `sapho-cli` | Checked command-line invocation and host-owned I/O, backends and exit policy |
| [evidence](modules/evidence/spec.md) | `sapho-evidence` | Curated labelled cases, reproducible measurements, development tuning and training exports |
| [selection](modules/selection/spec.md) | `sapho-select` | Bounded acquisition of identified file, Git-diff and JSON units outside the executor |
| [skills](modules/skills/spec.md) | `plugins/sapho` | Graph creation, tuning and recording workflows using the public CLI |

## Crate Dependency Boundaries

```mermaid
flowchart TD
 runtime[sapho-runtime] --> graph[sapho-graph]
 runtime --> core[sapho-core]
 graph --> core
 jev[sapho-jev] --> core
 jev --> systemone[sapho-systemone]
 clm[sapho-clm] --> core
 clm --> systemone
 systemone --> core
 recording[sapho-recording] --> core
 ollama[sapho-ollama] --> core
```

The root `sapho` package is an embedding facade over these crates; it owns no independent behavior and its optional `jev` and `clm` features are disabled by default. The logic specification module shares the runtime crate; logic operators have no transport or EARS dependency. The CLI host depends on the existing graph/runtime/recording adapters and the new pure `sapho-evidence` and host-I/O `sapho-select` crates. Evidence and selection depend on core; neither performs inference or imports the runtime. The CLI remains a synchronous process boundary around async engine execution. Original plugin assets under `plugins/sapho` invoke the CLI; they own no engine semantics. Core owns the shared ports so recording, Jev and Ollama need no runtime dependency. `sapho-ollama` depends only on core among workspace crates. Jev and CLM depend on `sapho-systemone` for their identical request translation and shared CLM model alias; systemone depends only on core among workspace crates and uses SDK-owned request/question types.

## Public Contract

`GraphSpec::parse` loads constrained YAML; `parse_with_format` chooses YAML or JSON explicitly. TOML graph consumption is removed without a compatibility layer. `compile` binds checked graph operations and native implementations. `Engine::run` accepts named Datum values and finite RunLimits, returning RunResult or RunFailure with partial Trace. Registries reject duplicate names. ModelBackend is the asynchronous inference seam for typed questions; Extractor is the asynchronous seam for one stateless call that returns one schema-checked structured record; Primitive is the synchronous native-code seam.

Each binding names a graph input, a node port, or a typed literal and may select a record-field path. Each operation declares its input/output port types. Guarded ports are Optional; consumers explicitly coalesce them. Model calls accept a Record state and ordered Questions and return validated Answers. Explicit ask nodes define batching; the executor never merges different ask nodes.

A measured model roster is a separate, versioned evidence artifact. The host observes actual backend calls and optional live elapsed durations without adding timing to deterministic Trace or recording. A roster identifies its Dataset and split and profiles each output under its contributing response's binding and actual model identity. A generated selection of its entries can be embedded as an ordinary typed Record literal; that literal participates in the roster's versioned semantic graph identity. The existing path/raw-byte `GraphArtifact.source` identity remains unchanged in ordinary reports.

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

Errors use a typed ErrorCode plus contextual fields, not message parsing: Config, DuplicateId, UnknownPrimitive, UnknownBackend, UnknownReference, Cycle, TypeMismatch, MissingInput, InvalidValue, InvalidAnswer, MissingAnswer, UnsupportedDistribution, LimitExceeded, DeadlineExceeded, TooLarge, CodeFailed, BackendFailed, Unauthorized, RateLimited, ServiceValidation, ModelMismatch, ReplayMiss, RecordingIo and RecordingMismatch.

## Probability Semantics

Probability, reported confidence, expected ordinal score and heuristic Degree are different quantities. Degree reductions do not claim joint probability or labelled correctness. Missing probability entries are not zero. Explicit host DistributionPolicy governs complete-distribution mass acceptance, is retained in Answers/requests/recordings, and never changes raw ModelResponse. Approximate complete projections normalize under that policy with an inspectable raw mass/scale; partial distributions are never normalized. Consumer thresholds, domain ambiguity labels and expert-selection policy remain downstream.

## Verification Strategy

Each functional artifact declares observable acceptance criteria and one planned TC. Integration artifacts exercise cross-crate seams. Tests bind exact AC IDs using `Trace:` tags. The EARS-shaped synthetic integration test establishes engine composition only, not semantic model quality. Default tests run offline.

## Lifecycle and Change Control

First-release requirements are specified and reviewed before code. Requirement IDs are global across the module directories. Changes update their owning artifact and affected tests. Implemented/verified status is reported from measured evidence rather than a copied ticket claim.

## CLI and Evidence Contracts

The initial command set is validate, inspect, run, record, replay, select (files/git/json), measure, tune and export-training. The roster command writes measured binding profiles and can project selected entries as a typed graph literal. Commands consume explicitly selected graph/input/binding/dataset paths. Machine output is typed JSON. Existing application-owned Rust registries remain the extension boundary; the stock executable never loads arbitrary code. Dataset curation requires Boolean labels whose provenance declares their kind (model, agent, human or deterministic_check) and source, explicit development/held_out splits and stable case identities. A model answer is a valid label when its kind and source are declared, and measure never scores a model against labels it made itself. Reports separate scored coverage from errors/unscored outputs. Only Probability is measured by Brier score, predicted-label ECE and risk coverage; Degree remains a heuristic. Empty scored sets have absent ECE and an empty risk table. Tuning ranks complete development candidates only; training export excludes held-out cases. Exact replay and raw evidence policy remain unchanged.

All approved acquisition and graph-skill behavior is specified before implementation. Generic selectors live outside the runtime and retain full selected bytes/context with source references. EARS owns its separate consumer migration, coordinated through SAPHO-4; no EARS data or extractor source is copied here. The skills create/tune/record operate through the same CLI and guide.

## References

The authoritative TypeSafe SDK is a package dependency. Native domain adapters and private evidence are supplied from their owning repositories, never copied into Sapho.

CLM and shared CLI foundations are specified in [CLM](modules/clm/spec.md), [provider credentials](modules/cli/functional/FR-045.md), [CLI foundations](modules/cli/functional/FR-046.md), the [Ollama provider](modules/cli/functional/FR-055.md) and the [large-Dataset ceilings](modules/cli/functional/FR-056.md). [Decisions contract acquisition](modules/clm/functional/FR-047.md) remains an external prerequisite; no wire schema is assumed.
