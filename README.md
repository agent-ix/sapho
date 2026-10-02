<div align="center">
  <img src="sapho-1.png" alt="Sapho logo" width="240" />
</div>

# Sapho

[![Discord](https://img.shields.io/badge/Discord-Join%20us-5865F2?logo=discord&logoColor=white)](https://discord.gg/6qsdhSPE)

**Build multi-layer evaluations from model judgments, Rust code and explicit logic.**

Sapho is a Rust library and command-line tool for making compound decisions with
System One models such as Jev. System One models cut inference time and cost
significantly, and they are fast enough to run on an M5 laptop.

Sapho splits a decision into many simple questions, asks the model each one, and
combines the answers using rules you write. Rules use logic operators such as
and, or, min and max, and you can build larger rules out of smaller ones.

![A graph mixes Rust preparation and facts with two model layers, then combines their results into findings and evidence.](docs/images/evaluation-flow.png)

The name comes from the juice Mentats drink to aid their mental calculations.

## What you can build

- **Code review:** extract code units, ask focused questions, combine judgments
  with static facts, and return advisory findings to a PR or editor workflow.
- **Requirement analysis:** classify statements, identify roles, evaluate
  candidate relationships and assemble findings across several model stages.
- **Semantic checks:** keep domain rules in configuration while choosing a
  backend for each question stage.

Your application supplies the extraction, questions and meaning of the result.
Sapho runs and connects them. Jev support is included as an optional feature;
you can implement the same backend interface for local Laya, KEV or other models.

## Build evaluations in layers

A layer can ask a model, execute Rust code or combine earlier evidence. Its
output can become the next layer's context, questions, candidate items or
execution condition. Branches let you run several checks and bring their
answers back together. Each model stage can use a different registered backend.

For example, a requirement evaluation can move from structure to roles, then
relationships, then a focused expert check. Rust primitives prepare each
stage; explicit logic decides which evidence is sufficient and which items
need another check.

![Illustrative requirement evaluation: structure, roles, candidate preparation, relationships, guarded expert checks and findings.](docs/images/multi-layer-requirements.png)

These are three ways to compose the same engine. The questions and policies
are examples you define in your application:

| Example | Layers and combinations | Walkthrough |
|---|---|---|
| Requirement analysis | Classify structure → identify actor/action/target → build candidate links → judge relationships → optionally ask an expert → assemble findings. | [Dependent model layers](docs/user-guide.md#example-1-requirements-through-several-model-layers) |
| Code review | Combine a Rust fact with two model judgments: `changed_public_api AND (contract_risk >= 0.7 OR test_gap >= 0.8)`. | [Hard facts plus soft evidence](docs/user-guide.md#example-2-hard-facts-plus-model-evidence) |
| A composite rule | Use minimum within required parts, maximum across acceptable alternatives, a weighted mean for supporting evidence, then a final threshold. | [Nested combinations](docs/user-guide.md#example-3-combine-combinations) |

You can mix these patterns in one graph. A native primitive can turn earlier
answers into a later question batch; `map` applies a reusable evaluation to
each item; guards select expensive checks. The graph is acyclic: an edit and
re-evaluation loop belongs to the calling application.

## Features

- **Multi-layer composition.** Feed earlier judgments into later questions,
  mix model stages with Rust transformations, branch into independent checks
  and combine the results. Use a different backend for each model stage.
- **Data CLI and graph skills.** Validate and run data-only graphs, gather files/Git patches/JSON, record and replay evidence, measure supplied labels, compare development candidates and export training cases. The plugin provides `create`, `tune` and `record` workflows.
- **Checked graph configuration.** Compilation checks connections, types and
  dependencies before any Rust function or model call executes.
- **Typed questions and answers.** Batch Boolean, choice and ordinal-score
  questions, then project the evidence you need for later stages.
- **Explicit logic.** Combine Boolean facts with `and`, `or` and `not`; combine
  heuristic strengths with minimum, maximum, weighted mean and complement.
  Apply thresholds with a comparator you choose.
- **Collections and dependent stages.** Map reusable graphs over items, filter
  results, form candidate pairs, join records and feed earlier answers into
  later questions. Guards skip work when a condition is false.
- **Evidence and replay.** Preserve item identity and source references, inspect
  intermediate results, and replay recorded model exchanges offline while
  trying a different combination or threshold.
- **Bounded execution.** Set ceilings for work, collection expansion, model
  calls, concurrency, serialized data and duration.

## How logic combines

Suppose a review rule needs three pieces of supporting evidence. After making
each interpretation explicit, you have strengths of **0.8, 0.4 and 0.6**.
The combination expresses your rule's policy:

- **Minimum → 0.4:** every part matters; the weakest part limits the result.
- **Maximum → 0.8:** the strongest supporting part is sufficient.
- **Weighted mean → 0.65:** balance the evidence, giving the first part twice
  the weight of each other part.

![Three input strengths combined by minimum, maximum and weighted mean.](docs/images/degree-combiners.png)

The result is a **heuristic degree**, a strength between zero and one. It is
not a calibrated probability that a compound statement is true. A separate
comparison turns that degree into a Boolean decision: for example,
`strength < 0.7` means "send this item for review." You choose both the
combination and the threshold.

Reducers can also feed other reducers: combine parts into a statement,
statements into a finding, and findings into an overall review policy.
`complement` reverses a degree; Boolean `not` reverses a decision.

The [logic walkthrough](docs/user-guide.md#combine-logic-and-evidence) explains
the formulas, types, empty inputs and complete graph configuration. The
[worked examples](docs/user-guide.md#worked-composition-examples) show how
several kinds of combination fit together.

## Start using Sapho

Use the CLI for data-only graphs, or embed the same engine in your Rust application:

```sh
cargo install --path crates/sapho-cli --locked
printf '%s' '{"support":0.7}' | sapho run examples/graphs/review.yaml --fail-on needs_review
```

Start with the [CLI guide](docs/cli-guide.md) for selectors, replay, labelled measurement, tuning and training export. Add `--features jev` when installing to enable explicitly configured live Jev calls.

Sapho is also an embedding library: call it from your Rust application, service,
CLI or review adapter. Use Rust 1.98 or later; this repository pins 1.98.1.
Depend on the Git repository with an account that has access:

```toml
[dependencies]
sapho = { git = "https://github.com/agent-ix/sapho.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For development alongside a checkout, use
`sapho = { path = "../sapho" }` instead. Pin a tested Git revision with `rev`
when you need a fixed dependency version.

The integration flow is:

1. Define input types, nodes and outputs in a YAML or JSON graph.
2. Register any Rust primitives and model bindings the graph names.
3. Parse and compile the graph, then construct an `Engine`.
4. Supply identified inputs and `RunLimits`, and await `engine.run(...)`.
5. Use the outputs in your application; retain the trace when you need evidence.

Start with the [complete offline example](docs/user-guide.md#run-your-first-graph).
It compares a supplied probability against a review threshold and needs no
model credentials. Then follow the
[model-question example](docs/user-guide.md#ask-model-questions) to produce
that evidence inside the graph.

Enable the adapter when you want Jev:

```toml
sapho = { git = "https://github.com/agent-ix/sapho.git", features = ["jev"] }
```

See [Jev setup](docs/user-guide.md#connect-jev) for client configuration and
[custom backends](docs/user-guide.md#use-another-model-backend) for other models.

## User guide and integration choices

The [CLI guide](docs/cli-guide.md) covers invocation, data gathering, evidence capture, measurement, tuning and training export. The [Rust user guide](docs/user-guide.md) covers installation, runnable graphs,
question types, logic, Rust extensions, multi-stage evaluation, source
references, limits, errors, recording and replay.

Use the `sapho` facade for normal integration: `sapho::core` supplies values and
extension traits, `sapho::graph` compiles configuration, `sapho::runtime` runs
it, and `sapho::recording` adds model recording and replay. The `jev` feature
adds `sapho::jev`.

If you only implement a model adapter, depend on `sapho-core`. If you only
validate graph configuration, use `sapho-graph`. The
[crate selection guide](docs/user-guide.md#choose-your-dependencies) explains
the smaller dependencies available for those integrations.

## Behavior reference

The user guide is the starting point for using Sapho. The
[behavior specifications](spec/spec.md) define the contracts behind it:
[values and answers](spec/modules/core/spec.md),
[graph configuration](spec/modules/graph/spec.md),
[execution](spec/modules/runtime/spec.md),
[logic](spec/modules/logic/spec.md),
[Jev translation](spec/modules/jev/spec.md) and
[recording and replay](spec/modules/recording/spec.md),
[CLI invocation](spec/modules/cli/spec.md),
[labelled evidence](spec/modules/evidence/spec.md),
[selectors](spec/modules/selection/spec.md) and
[graph skills](spec/modules/skills/spec.md).
Use these when checking a boundary condition or implementing an adapter.

Generate local API documentation with `cargo doc --no-deps --open`;
add `--features jev` for the Jev adapter.

## License and contributions

Sapho is licensed under [AGPL-3.0-or-later](LICENSE). For contributions, read
[CONTRIBUTING.md](CONTRIBUTING.md), [content rights](CONTENT_RIGHTS.md) and the
[CLA](CLA.md). Join us on [Discord](https://discord.gg/6qsdhSPE).
