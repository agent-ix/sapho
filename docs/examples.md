# Runnable examples and recipes

[Documentation index](index.md) · [Graph reference](graph-reference.md) · [API reference](api-reference.md)

Run from the checkout root. `sapho` below is the installed executable; without
installing, use `cargo run --locked -p sapho-cli --` followed by the same
arguments. Provider examples require the corresponding feature and service;
every reference graph can also run with the deterministic Rust tutorial backend.

## Examples by feature

```sh
cargo run --locked --example reference
cargo run --locked -p sapho-select --example acquisition
cargo run --locked -p sapho-evidence --example measurement
cargo run --locked -p sapho-clm --example clm_configure
```

The first command loads both encodings, checks expected outputs and node
statuses, records and replays each graph, and verifies distribution and failure
behavior. [cases.json](../examples/reference/cases.json) names each case's input
file and lists its expected outputs and skipped nodes. The tutorial
backend always returns fixed synthetic answers; these examples demonstrate
contracts and do not measure model quality.

| Example | Demonstrates | Expected result |
|---|---|---|
| [facts.yaml](../examples/reference/facts.yaml) / [JSON](../examples/reference/facts.json) | and/or/not, all comparators, record, projection, ordered list | both false, either true, reverse false; 2 < 3; ordered `["A","B"]` |
| [strengths.yaml](../examples/reference/strengths.yaml) / [JSON](../examples/reference/strengths.json) | degree, map, aligned weights, all reducers, complement, empty reduction | min 0.4, max 0.8, mean 0.65, concern 0.35, empty 0.2 |
| [collections.yaml](../examples/reference/collections.yaml) / [JSON](../examples/reference/collections.json) | map with capture, filter, Cartesian pairs, equality join, collect | one selected record, two candidate pairs, one joined pair, `["a","b","c"]` |
| [guards.yaml](../examples/reference/guards.yaml) / [JSON](../examples/reference/guards.json) | guard, Optional presence/absence, coalesce | disabled: absent + true default; enabled: present false |
| [questions.yaml](../examples/reference/questions.yaml) / [JSON](../examples/reference/questions.json) | Boolean/choice/score, batched ask, probability, multiple layers | tutorial masses 0.8, 0.75, 0.75; second-layer support 0.8 |
| [model-collection.yaml](../examples/reference/model-collection.yaml) / [JSON](../examples/reference/model-collection.json) | map model questions over records and filter original inputs | both synthetic records selected |
| [guarded-model.yaml](../examples/reference/guarded-model.yaml) / [JSON](../examples/reference/guarded-model.json) | skip an ask, default Answers, recover model name | disabled: zero calls, support 0, model `skipped`; enabled: support 0.8 |
| [native.yaml](../examples/reference/native.yaml) / [JSON](../examples/reference/native.json) | registered Primitive with params | `lamp` has byte length 4 |
| [reference.rs](../examples/reference.rs) | Complete Rust host, custom backend/primitive, limits, trace, capture/replay, strict/approximate/partial/unavailable distributions | all expected outputs and failure codes verified |
| [acquisition.rs](../crates/sapho-select/examples/acquisition.rs) | file globs/exclusions, typed JSON and sources; optional Git modes | four selected YAML files, two JSON records; missing member refuses |
| [measurement.rs](../crates/sapho-evidence/examples/measurement.rs) | dataset validation, agreement/Brier, ranking, coverage, held-out split, JSONL export | agreement 1.0, Brier 0.25, one held-out case, two export rows |
| [Jev configuration](../crates/sapho-jev/examples/jev_configure.rs) | SDK client to backend registry | compiles without inference; running needs SDK configuration |
| [CLM configuration](../crates/sapho-clm/examples/clm_configure.rs) | bounded backend construction and registry | prepares a loopback binding offline, without inference |

Every reference graph reads its input from `NAME-input.json` in the same
directory; the guard examples add `NAME-enabled-input.json` for the enabled
branch. Model results with a real provider vary; only the deterministic host
promises the listed synthetic results. Native graphs need registered code and
cannot run in the stock executable.

## Supplied facts and strengths

Inspect and run a complete graph without model calls:

```sh
sapho validate examples/reference/facts.yaml
sapho inspect examples/reference/facts.yaml
sapho run examples/reference/facts.yaml --input examples/reference/facts-input.json
sapho run examples/reference/facts.json --format json --input examples/reference/facts-input.json
sapho run examples/reference/strengths.yaml --input examples/reference/strengths-input.json
```

Reports contain typed Datums and a trace. `either.value` is a Boolean true;
`weighted_mean.value` is Degree 0.65. Numeric facts, Probability and Degree
have distinct types. Both strength and weight maps derive from the same sample
records so item IDs align.

For a shell decision, use the supplied threshold graph:

```sh
printf '%s' '{"support":0.7}' | sapho run examples/graphs/review.yaml --fail-on needs_review
```

It emits the report and exits 1. Support 0.8 or 0.9 yields false and exits 0;
configuration/execution refusal exits 2. Boolean combiners do not suppress
upstream calls; guarded nodes are the execution-control mechanism.

## Gather, map and select records

Select the [complete input document](../examples/data/selection.json) using its
[explicit schema](../examples/data/selection-schema.json):

```sh
sapho select json --input examples/data/selection.json --pointer /records \
  --schema examples/data/selection-schema.json --port items
```

The output is typed Inputs with two identified record occurrences and source
references. To ask a live provider about each item, create `bindings.yaml`:

```yaml
judge:
  provider: clm
  model: clm-latest
  distribution_policy: {kind: strict}
```

With a host-managed CLM service and the CLI built with `--features clm`:

```sh
sapho select json --input examples/data/selection.json --pointer /records \
  --schema examples/data/selection-schema.json --port items |
  sapho run examples/reference/model-collection.yaml --typed-input --bindings bindings.yaml
```

The mapped subgraph asks a Boolean question, compares P(true) against 0.7, then
filters the original items using their preserved IDs. Real selections depend
on the provider's answers. `cargo run --example reference` runs this same graph
with fixed P(true)=0.8 and selects both records offline.

For files or Git patches, the [file-context graph](../examples/graphs/files.yaml)
accepts selector records with `path`, `text` and `status`:

```sh
sapho select files --root examples/graphs --include '**/*.yaml' --exclude '**/review-conservative.yaml' |
  sapho run examples/graphs/files.yaml --typed-input
sapho select git --root . --mode working_tree
sapho select git --root . --mode staged
sapho select git --root . --mode revisions --base HEAD --head HEAD
```

The file command selects four files. Git results depend on tracked changes;
comparing HEAD with itself yields an empty collection. Git needs Unix and
Git 2.34+. To exercise all modes through the Rust API, run
`cargo run -p sapho-select --example acquisition -- --git`.

## Candidate pairs and relationships

```sh
sapho run examples/reference/collections.yaml --input examples/reference/collections-input.json
```

A cutoff capture keeps the lamp record (score 0.9), dropping the motor record
(score 0.3). Pairing the one kept record with two right records produces two
candidates. Joining on `key` keeps the lamp-to-lamp pair. Pair sources merge
both inputs and IDs derive from the input identities; downstream findings can
refer back to the pair. A nested text collection flattens to `a`, `b`, `c`.

Use `pairs` when your domain needs to judge possible relationships; `join`
when an exact known key determines the relationship. Duplicate keys yield all
matches, not an arbitrary first match. Fan-out shares the runtime item budget.

## Multiple layers and guarded work

The [questions graph](../examples/reference/questions.yaml) asks all three
question kinds in one batch. It projects Boolean support, selected role mass
and the mass at ordinal level `"2"`. A second context includes the text and
first-layer support before another batched call. To run it with a live binding:

```sh
sapho run examples/reference/questions.yaml --input examples/reference/questions-input.json --bindings bindings.yaml
sapho run examples/reference/guarded-model.yaml --input examples/reference/guarded-model-input.json --bindings bindings.yaml
```

The guarded input has `enabled: false`; the ask is skipped, coalesce supplies
fallback Answers with zero support and a model name `skipped`. Required backend
bindings are still prepared before evaluation. `guarded-model-enabled-input.json`
enables the ask, which performs one call. The offline runner verifies both
paths and confirms the disabled path records no exchanges. Default Answers must
retain the matching question space.

## Record, replay, measure and tune

For a complete no-provider workflow, use fresh artifact paths:

```sh
work_dir=$(mktemp -d)
printf '%s' '{"support":0.7}' > "$work_dir/input.json"
sapho record examples/graphs/review.yaml --input "$work_dir/input.json" \
  --recording "$work_dir/recording.json" --trace "$work_dir/trace.json" --output "$work_dir/report.json"
sapho replay examples/graphs/review.yaml --input "$work_dir/input.json" --recording "$work_dir/recording.json"
sapho measure examples/graphs/review.yaml --dataset examples/data/review-dataset.json --split development
sapho tune --candidate examples/graphs/review.yaml --candidate examples/graphs/review-conservative.yaml \
  --dataset examples/data/review-dataset.json --output-name needs_review --metric agreement
sapho measure examples/graphs/review.yaml --dataset examples/data/review-dataset.json --split held_out
sapho export-training --dataset examples/data/review-dataset.json --output "$work_dir/development.jsonl"
```

The graph returns needs_review=true, records zero model exchanges, and replays
the same output. The 0.8 threshold agrees with both development labels (1.0); the conservative
0.95 threshold agrees with one (0.5), so the 0.8 candidate ranks first. It
also agrees with the one
held-out label. Export writes two development JSONL rows and excludes the
reserved case. These are synthetic labels and demonstrate the workflow.
Artifacts are created exclusively; rerunning against the same paths refuses.

For model capture, replace the graph/input with the questions example and add
`--bindings bindings.yaml` to record. Replay needs only the saved input and
recording when binding identities are unique. Measurement/tuning can use
`--replay PATH` with recordings covering every case request. Changing downstream
thresholds preserves replay when requests stay the same; changing context,
questions, models or distribution policy causes a miss or metadata refusal.
The Rust runner performs actual model-boundary capture/replay using its tutorial
backend without contacting a service.

## Inspect expected failures

```sh
sapho run examples/reference/collections.yaml --input examples/reference/collections-input.json --max-items 1
sapho select json --input examples/data/selection.json --pointer /missing --schema examples/data/selection-schema.json
```

The first returns LimitExceeded with partial execution evidence and exits 2.
The second refuses a missing JSON member and exits 2. The Rust runner additionally
checks a wrongly typed filter mask (TypeMismatch), an empty-recording request
(ReplayMiss), a nonunit strict distribution (InvalidAnswer) and missing outcome
mass (UnsupportedDistribution). Fix the declared contract or exact replay input;
use structured codes instead of interpreting message text.

## Verify and extend documentation

```sh
make docs-check
```

This builds warning-free API docs, runs default/all-feature doctests, compiles
the Jev example without invoking it, runs the offline Rust examples (including
CLM backend construction) and CLI recipes, checks YAML/JSON equivalence and
checks local documentation links. Each reference graph needs an entry in the
[reference manifest](../examples/reference/cases.json) and an input file. The
reference runner maps every operation, comparator, reducer and question kind to
the case that runs it; that mapping has no wildcard arm, so a new variant does
not compile until it names a case whose graph uses it. List new operation,
question, comparator, reducer, provider and command variants in the
[coverage inventory](feature-coverage.md) as well.
