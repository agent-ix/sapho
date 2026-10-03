# Complete user and API documentation plan

## Goal

A CLI user or Rust integrator can discover every supported feature, understand
its contract, run a complete example, inspect the result, and diagnose common
failures without consulting implementation code or behavior specifications.

This is documentation work for existing behavior. API changes, new graph
operations, and a hosted documentation deployment are outside this plan.

## Baseline before implementation

The README links `docs/user-guide.md` and `docs/cli-guide.md`; rustdoc exists
across the facade and ten workspace crates. `make docs` builds the workspace
with all features and treats rustdoc warnings as errors. CI also runs tests
with all features and with no default features.

The checked-in examples contain five graphs and two data files. The guides
cover substantial ground, but many examples are snippets or conceptual flows
rather than independently runnable programs. The operation table alone does
not establish example coverage. The Rust guide still mentions TOML loading
although the graph parser supports YAML and JSON.

## 1. Inventory and coverage

Create `docs/feature-coverage.md` from public exports, the graph schema, CLI
command definitions, and owning specifications. Give each feature a reference
page, runnable example, expected result, and verification command. Mark gaps
explicitly until delivered.

Cover these areas:

- Core: typed IDs, values and schemas, Datum identity, source references,
  plain and typed JSON, question definitions, answers, distributions,
  probability versus Degree, signatures and structured errors.
- Graphs: YAML and JSON authoring, format selection, input/node/literal
  bindings, field projection, compilation, reusable subgraphs, dependencies,
  guards and Optional outputs.
- Every operation: record, list, code, questions, ask, map, filter, pairs,
  join, collect, and, or, not, compare, probability, degree, reduce,
  complement and coalesce. Cover all five comparators, all three reducers,
  Boolean/choice/ordinal questions, and strict/approximate distribution policy.
- Runtime: engine construction, registries, stages and concurrency, nested
  execution budgets, cooperative cancellation/deadlines, successful traces
  and partial failure traces.
- Integration: custom Primitive and ModelBackend implementations, Jev and
  CLM configuration and feature flags, transport/configuration boundaries,
  shared System One request conversion, and crate dependency choices.
- Recording: capture, snapshot, serialization, bounded persistence, exact
  replay matching, mismatches and offline execution.
- Selection: files, inclusion/exclusion patterns, Git modes, JSON selection,
  identities/sources and acquisition limits.
- Evidence: dataset schema and splits, labels, prediction outcomes, metrics,
  unscored cases, candidate ranking and training export.
- CLI: every command and option, bindings and credential resolution,
  input/output formats, artifact paths, limits, exit statuses and plugin use.

Completion: no supported feature or public API family lacks a documented
entry and an example. Individual public symbols retain rustdoc coverage.

## 2. Build a navigable reference

Add `docs/index.md` linking quickstarts, task guides, reference and examples.
Keep the README focused on installation and the first successful run.

Add a graph reference with exact configuration fields, defaults, required
ports, output types, constraints and examples for every operation. Explain
identity alignment, empty collections, join multiplicity, guard propagation
and reducer weight rules at the relevant operation.

Add a CLI reference for every command, option, default, output and exit code.
Link recipes to it rather than duplicating complete flag descriptions.

Add an API landing page mapping facade modules and leaf crates to use cases.
Expand crate/module rustdoc and public API documentation with prerequisites,
parameters, results, errors, feature requirements and runnable examples.
Explain that selection, evidence and the shared codec are separate crates,
rather than suggesting they are facade re-exports.

Correct stale guide text against current code and the owning specification.
Link Rust APIs to generated rustdoc; provide exact local build/open commands
for default and optional adapters. Choose hosting only in a separate task.

## 3. Deliver runnable examples

Extend `examples/graphs` and `examples/data` with small deterministic cases
for every graph operation and variant. Include expected outputs and an
example index with copyable commands. Supply JSON equivalents where they
teach format differences; both representations must be verified.

Add complete Rust examples for embedding, custom primitives, a deterministic
backend, collections/subgraphs, guard/coalesce, limits/failure traces,
recording/replay, selection, measurement/ranking and training export. Put
leaf-crate examples with their owning crates where that avoids adding
production dependencies to the facade.

Use a deterministic backend at the model seam for executable examples of all
question types and distribution policies. Keep Jev/CLM live examples clearly
labelled with feature, service and credential prerequisites; compile them
without making network calls during normal checks.

Add end-to-end recipes for:

1. Supplied facts and strengths producing a review decision.
2. Selected records mapped through model questions, filtered and aggregated.
3. Candidate pairs and joins producing attributable relationship findings.
4. Multiple model layers with guarded expensive work.
5. Record once, replay offline, compare thresholds on development labels,
   then measure the selected graph on held-out cases and export training data.

Each recipe supplies all files, exact commands, expected outputs and an
explanation of the policy choices. Include useful failure examples, such as
type mismatch, identity mismatch, invalid distribution, limit exceeded and
replay miss, with the corresponding structured error or exit status.

## 4. Verify and prevent drift

Add a bounded offline documentation check that validates and executes all
example graphs and checks meaningful expected outputs and failure codes.
Compile/run Rust examples and rustdoc examples under the appropriate feature
sets. Exercise collection identities, empty cases, guards and replay request
matching rather than merely checking that commands exit successfully.

Prefer executable source files as the canonical examples. Where guides copy
code, verify synchronization or generate the copied regions. Check local
Markdown links and referenced example paths. Reuse existing test helpers and
build targets where possible.

Wire the checks into the existing Makefile and manually dispatched CI. Run
`make docs`, relevant example/doctest checks, formatting and linting, plus
the existing default/all-feature tests when check infrastructure changes.
Normal verification requires no credentials, network or model server.

## Delivery order and acceptance

Deliver inventory/navigation first, then core/graph/runtime reference and
examples, then adapters/recording/selection/evidence/CLI coverage, then the
end-to-end recipes and drift checks. Update coverage with each increment.

The work is complete when every inventory entry links to accurate reference
and a verified example; all offline recipes reproduce their documented
results from a clean checkout; optional provider examples compile; rustdoc
builds without warnings; and documentation links and example checks pass.

## Implementation delivered

The [documentation index](index.md) now links the API, graph and CLI references,
[feature inventory](feature-coverage.md), [runnable recipes](examples.md) and
generated command help. Eight dual-encoding graph cases cover every operation,
comparator, reducer and question kind; a Rust host checks their actual outputs,
distribution policies, guards, structured failures and exact recording/replay.
Leaf-crate examples cover selection, scoring/ranking/export and provider setup.
Every crate has a compiled usage doctest. `make docs-check` and CI verify the
examples, references, local links, coverage inventory and CLI help drift.
Live provider inference and Claude plugin loading require their respective
external hosts; optional provider construction examples are compiled offline.
