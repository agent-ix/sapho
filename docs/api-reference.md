# Rust API reference

[Documentation index](index.md) · [Rust guide](user-guide.md) · [Complete examples](examples.md)

## Crates and generated API documentation

Use the `sapho` facade for graph embedding. It re-exports `core`, `graph`,
`runtime`, `recording`, optional `jev` and optional `clm`. Selection, evidence
and the shared System One codec are separate dependencies. CLI hosts can use
`sapho-cli` as a library. The default facade has no model provider enabled.

```toml
[dependencies]
sapho = { path = "../sapho", features = ["clm"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
# Add only if your host gathers input or evaluates labelled datasets:
sapho-select = { path = "../sapho/crates/sapho-select" }
sapho-evidence = { path = "../sapho/crates/sapho-evidence" }
```

Adjust paths to your checkout. For Git dependencies use the repository URL
and select the package name. Features `jev` and `clm` are independently optional
on both the facade and executable.

Build the complete symbol-level API reference locally:

```sh
make docs
cargo doc --locked --workspace --no-deps --all-features --open
```

`make docs` writes each package's generated reference under `target/doc` in
the checkout; open the page listed for a package in a browser:

| Package | Generated reference after `make docs` | Main contracts |
|---|---|---|
| `sapho` | `target/doc/sapho/index.html` | Convenient embedding imports |
| `sapho-core` | `target/doc/sapho_core/index.html` | Values, IDs, models, ports, registries, errors |
| `sapho-graph` | `target/doc/sapho_graph/index.html` | Config schema, parser and compiler |
| `sapho-runtime` | `target/doc/sapho_runtime/index.html` | Engine, budgets and traces |
| `sapho-recording` | `target/doc/sapho_recording/index.html` | Explicit capture and exact replay |
| `sapho-select` | `target/doc/sapho_select/index.html` | Bounded synchronous host acquisition |
| `sapho-evidence` | `target/doc/sapho_evidence/index.html` | Pure scoring, ranking and training export |
| `sapho-systemone` | `target/doc/sapho_systemone/index.html` | Source-free SDK request translation |
| `sapho-jev` | `target/doc/sapho_jev/index.html` | Hosted SDK adapter |
| `sapho-clm` | `target/doc/sapho_clm/index.html` | Bounded HTTP adapter and transport seam |
| `sapho-cli` | `target/doc/sapho_cli/index.html` | Inspection, invocation, bindings and artifacts |

The tables below explain how to use the APIs together. Generated rustdoc
provides exact signatures, fields and variants for individual symbols.

## Values and interchange

| API | Use and constraints |
|---|---|
| `NodeId`, `ItemId`, `BackendId`, `PrimitiveId`, `SourceId` | Distinct opaque identity types. `new` rejects empty strings; `as_str` borrows text; `validate` checks deserialized identities. |
| `Probability`, `Degree` | `new(f64)` rejects nonfinite/out-of-range values; `get()` reads the scalar. Both serialize as numbers but have different semantic types. |
| `ValueType` | Boolean, Number, Text, Probability, Degree, Optional, List, Record, Questions, Answers. `list`/`optional` construct nested types; `validate` bounds nesting/names; `check` enforces exact schema. |
| `Value` | Corresponding typed payloads. `validate` checks nested invariants; `to_plain_json` removes Datum identity/source sidecars. |
| `Datum` | `new(id, value)` validates; `validate` checks loaded data; `inherit_sources` merges/deduplicates attribution. |
| `SourceRef` | `source: SourceId`, `start`, `end`. Both offsets absent or present, half-open bytes with start ≤ end. `validate` never opens a source. |
| `Inputs`, `Signature` | `BTreeMap<String, Datum>` and exact named input/output schema maps. |
| `validate_name`, `check_ports` | Validate names and exact named port membership/types; extra ports are errors too. |
| `decode_json<T>` | Strict bounded JSON decode; rejects duplicate keys and malformed/deep input. Call the decoded object's validation method where needed. |
| `decode_plain` | Decode ordinary JSON against a ValueType into a Datum with the given ID and source references. List occurrences get stable IDs derived from that ID and their index. |
| `bounded_json`, `measured_json_bytes` | Serialize or count bytes under a ceiling; return `LimitExceeded` when exceeded. |

A typed input preserves identity:

```json
{
  "support": {
    "id": "requirement-7",
    "value": {"kind": "probability", "value": 0.7},
    "sources": [{"source": "requirements", "start": 120, "end": 173}]
  }
}
```

Plain input for the same graph is `{"support":0.7}`. The graph's input schema
decides whether a number means Number, Probability or Degree. Optional uses
`null` for absence in plain JSON and `{kind: optional, value: null}` in typed
JSON; present typed Optional contains another typed Value. Record fields are
Values, whereas List items are Datums. Exact record field membership is checked.
Two occurrences with equal text need different item IDs. Source attribution is
carried through execution but not automatically inserted into model state.

The [reference runner](../examples/reference.rs) shows schema-directed decoding
and conversion of final outputs to plain JSON. The
[acquisition example](../crates/sapho-select/examples/acquisition.rs) demonstrates
source-bearing inputs.

## Questions, answers and distributions

| API | Contract |
|---|---|
| `NamedQuestion`, `Question`, `ChoiceOption` | Ordered identified Boolean, Choice and Score definitions; `Question::labels()` gives their outcome spaces. |
| `validate_questions` | Reject duplicate/empty IDs, blank instructions and invalid choice/score cardinality or labels. |
| `Answer` | Boolean probability; Choice selected label/confidence/optional distribution; Score expected value/confidence/optional distribution. |
| `ModelRequest` | Backend ID, requested model, optional expected model, distribution policy, Record state and ordered questions. `validate()` checks before transport. |
| `ModelResponse`, `Usage` | Actual model and raw typed answers; optional input/output tokens and separate billing units. |
| `validate_response` | Checks request, actual model identity, exact answer IDs/types/labels/masses; returns validated Answers retaining question definitions. |
| `Answers::validate` | Check native or loaded answer data without inference. |
| `Answers::probability` | Project known outcome masses from a question; unknown labels refuse and unavailable mass is never assumed zero. |
| `distribution_state`, `DistributionState` | Complete, Approximate, Partial or Unavailable per question. |
| `distribution_adjustment`, `DistributionAdjustment` | Inspect raw total mass and derived normalization scale for approximate full distributions. |
| `DistributionPolicy` | `Strict {}` allows mass roundoff of 1e-6. `approximate(error)` allows positive absolute full-mass error ≤ 0.05 (plus roundoff). `validate` checks policy bounds. |

Boolean answers report P(true); P(false) is its complement. Choice `selected`
and `confidence` do not replace its distribution. A Score's `expected` is on
the configured ordinal scale, not `[0,1]` unless the rubric has two levels.
Score outcome labels are decimal indices. Partial distributions can project
present labels and never normalize missing outcomes. A complete nonunit
mass distribution requires approximate policy; only its projections normalize,
and the raw response stays unchanged.

For example, masses 0.7 and 0.29 fail strict validation. Under allowance 0.02,
actor probability becomes `0.7 / 0.99`; raw masses remain intact. The
[reference runner](../examples/reference.rs) verifies this, partial and absent
mass, and all three question types. The
[questions graph](../examples/reference/questions.yaml) shows one batched call
and a second call whose state includes prior support.

## Graph parsing and compilation

`GraphSpec::parse(text)` reads YAML. `parse_with_format(text, GraphFormat::Json)`
reads JSON explicitly; generic `parse_config<T>` applies the configuration
profile to other typed documents. `GraphFormat` has YAML and JSON variants.
`GraphBody`, `NodeSpec`, `Binding`, `Operation`, `Comparator` and `Reducer`
form the public authoring vocabulary; see the [graph reference](graph-reference.md).

`compile(&spec, &primitives)` returns an immutable `CompiledGraph`, rejecting
invalid names/types/ports, unknown references or primitives, dependency and
subgraph cycles, and compiler budgets. It does not call native code or models.
`CompiledGraph::signature()` exposes root ports, `stages()` dependency stages,
and `outputs()` root output bindings. A `CompiledNode` exposes `spec`,
`signature`, `output_types`, its resolved `primitive` and its `mapped_graph`.

Register a Primitive before compilation. It implements `signature()` and
synchronous `execute(context, inputs, params) -> Result<Inputs>`; check
`PrimitiveContext::check_cancelled()` during bounded native work. The runtime
runs native code through its blocking bridge. Register with
`PrimitiveRegistry::register(PrimitiveId, Arc<dyn Primitive>)`; duplicates refuse
and `get` returns `UnknownPrimitive` for missing implementations. Compilation
captures the signature; execution validates returned ports. The
[TextLength host](../examples/reference.rs) is a complete implementation.

## Execution, traces and errors

Register asynchronous providers with `BackendRegistry::register(BackendId,
BackendBinding)`. A binding contains `Arc<dyn ModelBackend>`, requested `model`,
optional `expected_model` and explicit `distribution_policy`. Duplicate IDs
refuse. `get` resolves by name. Implement `ModelBackend::infer(&ModelRequest)
-> Result<ModelResponse>` with `async_trait`; return raw typed answers and
let the engine validate them. The tutorial backend in the
[reference runner](../examples/reference.rs) is deterministic and makes no
semantic accuracy claim.

`Engine::new(compiled, backends)` verifies required backends before work.
`engine.run(&inputs, limits).await` returns `RunResult {outputs, trace}` or
`RunFailure {error, trace}`. Compiled signatures are immutable. The same engine
can evaluate multiple input sets. Evaluation does not persist evidence.

`RunLimits` requires positive fields: `node_instances` (default 4096),
`collection_items` (16384), `model_requests` (128), `concurrency` (4),
`data_bytes` (8388608), `duration` (60 seconds). Counters apply across nested
maps, including intermediate data and expansion. They bound work and serialized
data, not process memory. Model futures are dropped on deadline/failure;
native work must cooperate and cannot be forcibly preempted after starting.
No implicit retry occurs. Only model calls run concurrently: the ready `ask`
requests of a dependency stage are sent in chunks of at most `concurrency`.
Native code, logic and collection operations run one node at a time, and a map
evaluates its items in order. Output and trace ordering are deterministic.

`Trace.nodes` contains `NodeTrace`: scoped path, dependency paths, guard,
operation, inputs, outputs, `NodeStatus` (Completed/Skipped/Failed), optional
error and optional `ModelEvidence`. Model evidence retains the request and
available raw response, even when response validation fails. A failure preserves
earlier trace entries. It is typed evidence rather than raw HTTP traffic.

`SaphoError` contains an `ErrorCode`, message and deterministic contextual map.
Construct with `new`, enrich with `with_context`, and branch on code. Core's
`Result<T>` uses this error. Selection, evidence, CLI and CLM preparation have
additional boundary-specific error enums in their generated APIs.

| ErrorCode family | Typical trigger and response |
|---|---|
| `Config`, `DuplicateId`, `UnknownPrimitive`, `UnknownBackend`, `UnknownReference`, `Cycle` | Correct graph/registry configuration or colliding data identities. |
| `TypeMismatch`, `MissingInput`, `InvalidValue` | Fix exact port schema/membership, item alignment, numeric range or parameters. |
| `InvalidAnswer`, `MissingAnswer`, `UnsupportedDistribution`, `ModelMismatch` | Check provider answer contract, available mass and actual model identity. |
| `LimitExceeded`, `DeadlineExceeded` | Inspect partial trace and fan-out; supply appropriate finite budgets and cooperative native code. |
| `CodeFailed`, `BackendFailed` | Application primitive or transport refused; handle at the host boundary. |
| `Unauthorized`, `RateLimited`, `ServiceValidation` | Check host credentials, provider capacity or rejected request. No automatic retry. |
| `ReplayMiss`, `RecordingIo`, `RecordingMismatch` | Restore exact request/binding identity or fix recording persistence/consistency. |

The runner verifies `TypeMismatch`, `LimitExceeded`, `InvalidAnswer`,
`UnsupportedDistribution` and `ReplayMiss`. The [Rust guide](user-guide.md#inspect-outputs-and-failures)
shows application failure handling with partial traces.

## Recording and replay

`RecordingBackend::new(delegate, max_bytes)` decorates any model backend.
Retain an Arc while registering it; after evaluation `snapshot()` returns a
`Recording {exchanges: Vec<Exchange>}`. Each Exchange retains an exact
ModelRequest and successful validated raw ModelResponse. Invalid responses
remain in traces, not successful recording exchanges. Snapshots preserve the retained exchanges.

`Recording::validate` checks exchanges. `to_json`/`from_json` enforce byte
ceilings. `write_new(path, max_bytes)` exclusively creates a file and refuses
overwrite; `read(path, max_bytes)` loads explicitly. Both are synchronous host
actions; use them before/after async execution or through a blocking bridge.

`ReplayBackend::new(&recording, max_bytes)` implements ModelBackend using exact
request lookup. Request identity includes backend, model, expected model,
policy, state and ordered question definitions. Matching requests can be
reused; replay is not a consumable FIFO. Conflicting responses for the same
request refuse. There is no live fallback. Changing only downstream policy
thresholds can preserve replay compatibility.

Every graph in the [reference runner](../examples/reference.rs) is recorded,
serialized, loaded and replayed, comparing actual outputs. Model graphs also
verify ReplayMiss against an empty recording. The [recipes](examples.md#record-replay-measure-and-tune)
show exclusive file persistence and CLI replay.

## Selection APIs

Add `sapho-select` directly. All acquisition APIs are synchronous; acquire
before invoking an async engine or use a host blocking bridge.

- `select_files(root, patterns, limits, port) -> Result<Inputs, SelectionError>`
  gathers sorted regular text files under an explicit directory.
- `select_git(root, options, patterns, limits, port)` gathers complete tracked
  patches. `GitOptions` configures the executable and `GitMode`:
  WorkingTree, Staged, or Revisions with explicit base/head commits.
- `select_json(bytes, pointer, schema, id_prefix, port, location, max_bytes)`
  projects RFC 6901 and decodes against ValueType without inference.
- `Patterns {include, exclude}` supplies root-relative globs;
  `SelectionLimits::validate` checks positive ceilings. Defaults: files 1024,
  entries 10000, file bytes 1048576, total bytes 4194304, stderr bytes 65536,
  duration 30 seconds. `Resource` distinguishes exceeded ceilings.

File/patch records carry `path`, `text` and `status` Text fields, occurrence identity
and source evidence. JSON selected items retain generated identities and a
whole-document source reference. Selection errors distinguish invalid root,
nontext/unsupported file, bad glob/path encoding, pointer syntax/missing
member/index/type, Git version/revision/metadata/process failures, unsupported
platform, core validation, budgets and deadlines. Never parse diagnostic text.
See the [complete acquisition example](../crates/sapho-select/examples/acquisition.rs).

## Evidence APIs

Add `sapho-evidence` directly. It performs no I/O, inference or graph execution.
Your host supplies actual per-case outputs or failures.

| API | Use |
|---|---|
| `Case`, `Dataset`, `Split`, `LabelProvenance`, `LabelKind` | Identified typed inputs, Boolean output labels, label provenance (kind `model`, `agent`, `human` or `deterministic_check`, a nonblank source and reference, and for models an optional weights digest), development/held-out partition. Dataset `validate(max_cases)` checks all cases; `selected(split)` iterates a partition. |
| `CaseOutcome` | Completed Inputs or Failed SaphoError, each with the models that answered during the case, keyed by ItemId. |
| `measure(dataset, split, schemas, outcomes, max_cases)` | Return Measurement with per-output coverage and predictions against supplied labels. |
| `Measurement::complete()` | No failed or unscored labels; inspect selected count and denominators as well. `Measurement.self_source` lists cases whose model-made labels came from a model that also answered them (by name or weights digest); they are scored on no output and counted in no label total. |
| `OutputMeasurement`, `Prediction`, `UnscoredReason` | Labelled/scored/unscored/failed counts; retained values, provenance, missing/unsupported/type mismatch reasons and errors. |
| `Metrics`, `Confusion` | Boolean agreement and TP/TN/FP/FN; Probability Brier; Unsupported schemas. No metrics for Degree. |
| `Candidate`, `Metric`, `rank` | Rank complete development measurements for one output. Agreement descends, Brier ascends; stable ties. |
| `RankedCandidate` | Candidate ID, original input index and score; your host retains graph artifacts. |
| `TrainingRow`, `export_training` | Bounded JSONL development supervision preserving inputs, labels and provenance; excludes held-out cases. |
| `EvidenceError` | Core errors, duplicate case, missing labels/provenance, case/candidate limits, empty split, no candidates or no rankable candidate. |

Labels are supplied supervision, not model self-assessment. Unsupported or
missing outputs are unscored, not silently converted into predictions. Outputs
have separate denominators and are never averaged together. Ranking requires
all development cases to carry and score the selected output. Training export
does not convert supervision into provider-specific prompts or train a model.
The [measurement example](../crates/sapho-evidence/examples/measurement.rs)
verifies agreement, Brier, incomplete coverage, held-out isolation and export.

## Model adapters

**Jev:** add facade feature `jev` or depend on `sapho-jev`. Supply a configured
TypeSafe SDK `Client` to `JevBackend::new`. The synchronous host owns auth,
endpoint and transport; graph metadata owns model selection. Each inference
disables SDK retries. Boolean Noul maps to P(true); choice and score preserve
reported confidence and outcome masses. Usage retains token counts. HTTP
401/403 becomes Unauthorized, 429 RateLimited, 400/422 ServiceValidation;
transport/deadline errors remain typed and provider bodies are omitted from
retained diagnostics. The [configuration example](../crates/sapho-jev/examples/jev_configure.rs)
compiles without making a request; running it requires SDK environment setup.

**CLM:** add facade feature `clm` or depend on `sapho-clm`.
`ClmBackend::new(base_url, optional_secret, Limits)` prepares bounded HTTP.
Defaults: `DEFAULT_BASE_URL` is loopback port 8700, `DEFAULT_MODEL` is
`clm-latest`; Limits use 30 seconds, request bytes 1048576, response bytes
8388608 and in-flight 4. Queue time shares the deadline. URL preparation appends
`/v1/systemone`; HTTPS is required except loopback. Embedded credentials,
queries and fragments refuse. Retries and redirects are disabled.
`ConfigurationError` distinguishes endpoint, credential, limits and transport
preparation. `with_transport(Arc<dyn Transport>, limits)` replaces only the
native HTTP seam. `Transport::post` must bound body collection and return an
`HttpResponse {status, body}` without secret-bearing diagnostics. See the
[configuration example](../crates/sapho-clm/examples/clm_configure.rs) and
[live setup](cli-guide.md#bind-clm-explicitly).

CLM confidence is retained independently of outcome mass; its usage can contain
billing units separately from tokens. Sapho does not install/start the service.
Both providers use core validation and the shared request translator.

**Codec:** `sapho_systemone::build_request(&ModelRequest)` validates and returns
an SDK SystemOneRequest, translating Boolean, Choice and Score criteria and
plain record state without source sidecars. It performs no transport. Jev
also re-exports this function. `CLM_DEFAULT_MODEL` is the codec's shared alias.

**Ollama:** depend on `sapho-ollama` (not part of the facade).
`Server::new(base_url, Limits)` prepares one endpoint; `DEFAULT_BASE_URL` is
loopback port 11434 and `Limits` default to 600 seconds, 4 MiB requests and
16 MiB responses. At most one request is in flight in the process, across every
binding and embedder; the timeout covers waiting for that permit. URLs with a
scheme other than http(s), userinfo, a query or a fragment refuse, and neither
retries nor redirects happen. `OllamaBackend::new(server, Settings {model, think,
num_ctx, num_predict})` is one model binding; it refuses a zero limit or
`num_predict >= num_ctx`. It implements `Extractor` (call it through
`sapho_core::extract`, which checks the request and validates the answer
against its JSON Schema) and `ModelBackend` (one request per question block,
with probabilities derived from the answer tokens' log-probabilities; needs
`think: false`, at most ten Score levels, and answer values that begin with
different bytes). Prompts are never shortened: the server counts tokens, and a
prompt that does not fit is `TooLarge` with the counts. Every response carries
the weights digest observed at `/api/show` before the call. Ollama does not
attest the answering weights in generate or embed replies. Treat the digest as
answering-weights provenance only while model writes to that server are
exclusively controlled throughout the exchange. `OllamaEmbedder`
calls `/api/embed` through the same permit. `ScriptedExtractor` in core is the
test double for hosts that depend on `Extractor`. The
[live check](../crates/sapho-ollama/examples/live_smoke.rs) is run on demand
and is not part of the default tests.

## CLI embedding APIs

Use `sapho-cli` when your Rust host needs stock reporting and exit policy:

- `inspect` returns an Inspection of signature, root/mapped stage groups and
  required backend/primitive names. `plain_inputs` decodes ordinary input JSON.
- `Runner::new` accepts explicit registries; `Runner::inspection` exposes its
  compiled contract and async `run` returns a RunReport containing an ExitStatus.
  ExitStatus's `code` gives 0/1/2. `RunReport` carries outputs/trace/error.
- `Bindings`, `BindingConfig`, `Provider` describe supported stock providers.
  `live_bindings` prepares only required providers; `recording_bindings` wraps
  required bindings; `replay_bindings` infers or verifies exact identities.
  `Provider::credential_environment()` identifies its environment key, and is
  `None` for `Provider::Ollama`, which needs no credential; its generation
  members are `BindingConfig::ollama` (`OllamaOptions`).
- `resolve_credential` accepts an explicit secret, environment snapshot and
  SecretStore, resolving in that order. `resolve_endpoint` resolves explicit,
  environment and default values. These are synchronous host configuration.
- `select_format`, `load_graph`, `GraphArtifact`, `read_bytes` and
  `read_bytes_with_timeout` implement bounded regular-file/config loading.
  `ArtifactWriter::create` claims a fresh path, `finish` writes bounded bytes;
  `write_new` combines these. They are synchronous host I/O.
- `CliError` distinguishes core, boundary, file, argument/provider and
  credential failures; consult its variants rather than parsing messages.

The process [CLI reference](cli-reference.md) describes invocation options.
The engine is the simpler integration point if you do not need stock reports
or CLI persistence behavior.
