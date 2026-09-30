# Sapho

Sapho composes model judgments, host Rust functions and explicit logic in a typed,
bounded graph. The name comes from the juice Mentats drink to aid calculation.
This is an embedding library: consumers own extraction, questions, domain
interpretations, review invocation and enforcement.

## Workspace boundaries

| Crate | Responsibility | Internal dependencies |
|---|---|---|
| `sapho-core` | Values, source spans, question/answer contracts, primitive and backend ports | none |
| `sapho-graph` | TOML definitions, type checking, dependency ordering | core |
| `sapho-runtime` | Execution, guards, collection operations, logic, limits, traces | graph, core |
| `sapho-jev` | Hosted System One translation through TypeSafe SDK | core |
| `sapho-recording` | Explicit bounded recording and exact offline replay | core |
| `sapho` | Embedding facade; optional `jev` feature | graph, runtime, recording, core; optional Jev |

Logic is a specification module within the runtime crate. Local Laya/KEV support
can implement `ModelBackend`; no local model runner is included in this version.

## Embedding

Depend on `sapho` through its local checkout or private Git repository. Register
Rust `Primitive` implementations with typed input/output signatures. Parse a
consumer-owned TOML graph using `GraphSpec::parse`, then `compile` with the
primitive registry. Compilation checks every definition without executing code
or inference. Supply named `BackendBinding`s to `Engine::new`, then call `run`
with identified inputs and explicit `RunLimits`.

Every code node names a registered primitive; graph config cannot load a plugin,
execute a script or read a path. Native code can create later `Questions` from
earlier `Answers`. One `ask` node is one ordered question batch with one Record
state. Separate ready asks run within the configured concurrency ceiling.

This complete TOML graph negates a Boolean input:

```toml
[inputs.fact]
kind = "boolean"

[[nodes]]
id = "negate"
[nodes.operation]
kind = "not"
[nodes.inputs.value]
kind = "input"
name = "fact"

[outputs.result]
kind = "node"
node = "negate"
port = "result"
```

See the compiled [embedding example](src/lib.rs),
[public API integration scenarios](tests/engine.rs), and
[synthetic consumer composition](tests/scenarios/ears.rs). The consumer test
extracts phrase occurrences, classifies roles, filters by identified masks,
forms actor/action pairs, asks dependent relationship questions and assembles
selected edges. All those domain primitives live only in the test harness.

## Values and logic

`Datum` carries an item identity, typed value and opaque source spans. Repeated
text retains distinct IDs. Maps preserve item order and identity; filters align
Boolean masks by ID; Cartesian pairs and many-to-many keyed joins preserve
left/right order; collect flattens one level and rejects identity collisions.
Record-field projections are explicit bindings.

Boolean facts, probabilities, heuristic degrees and Optional absence have
distinct types. Guards produce Optional outputs; `coalesce` replaces only
absence. Choice distributions remain complete, partial or unavailable. A
projection that requests missing mass fails instead of inventing a probability.
Expected ordinal scores may be fractional. Conversion into Degree is explicit.
Min, max, complement and weight-normalized mean are heuristic operations;
they do not establish calibrated truth. Reductions declare an empty value and
thresholds use an explicit scalar comparator, including the equality boundary.

## Backends, evidence and bounds

With feature `jev`, construct `JevBackend` from a host-configured SDK client.
The host owns credentials, endpoint, requested model and optional strict actual
model identity. Each graph inference has one attempt; Sapho disables SDK
retries. No live service, credential or model download is needed for tests.

`run` returns outputs plus ordered trace evidence, or a failure plus a partial
trace. Requests and available raw typed responses remain visible in the trace,
including answer-validation failures. Jev diagnostics omit provider bodies that
could echo authorization. The caller owns sensitive input handling and decides
whether to export any evidence.

`RecordingBackend` decorates a backend with a finite in-memory ceiling. Export
and synchronous file I/O are explicit. `ReplayBackend` has no live delegate and
matches the complete reconstructed request, including state, option order and
model binding. Re-run the same executor with replay to compare logic changes.
Changed model questions/context cause `ReplayMiss`. Conflicting saved answers
for an identical request are refused; this first version cannot replay a
nondeterministic sequence of identical requests.

Limits apply across mapped work: node instances, collection expansion, model
requests, concurrent work, cumulative serialized data bytes and monotonic run
duration. Definitions are capped at 4096 nodes, map nesting at 16, value/type
nesting at 32 and TOML input at 1 MiB. Count expansion is checked before
allocation; bounded serialization avoids an oversized JSON scratch buffer.
These are work/data accounting ceilings, not a process-memory sandbox.
Host-native functions run off async workers, must cooperate with cancellation,
and remain responsible for allocations inside their own implementations.
Dropping pending model futures cancels their awaited work.

## Specification and checks

The [full master spec](spec/spec.md) links six module specs and 29 functional
requirements. The base, integrity, scope-boundary and dependency reviews were
completed before implementation. Tests carry acceptance-criterion `Trace:` tags.
The final Rust review is under `reviews/`.

Rust is pinned in `rust-toolchain.toml`. `make test` runs both feature lanes;
`make ci` runs formatting, both Clippy/test lanes, supply-chain checks, unsafe
checks, docs and local Quire validation. `make build` builds the workspace in
release mode. All commands use the workspace's `target/` directory. CI retains
the scaffold's manual invocation policy. Spec validation uses the locally
installed Quire tool; Rust CI does not assume an unpublished spec-tool install.

## License and contributions

AGPL-3.0-or-later. Read [content rights](CONTENT_RIGHTS.md),
[contributing](CONTRIBUTING.md) and the [CLA](CLA.md) before contributing.
Private examples, model recordings and model weights are not repository assets.
