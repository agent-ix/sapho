# Use Sapho from the command line

[Documentation index](index.md) · [Command reference](cli-reference.md) ·
[Runnable examples](examples.md)

Sapho connects model questions, typed data and explicit logic in a configurable graph. Use YAML to author a graph and JSON for generated programs and data. The CLI invokes the same compiler and engine as Rust embedding, so a graph has the same behavior in either host.

The [embedding guide](user-guide.md) explains questions, combiners, collections, guards and custom Rust extensions. Start here when your inputs and operations are data-only.

## Install and try an offline rule

Use the repository's Rust 1.98.1 toolchain. From a Sapho checkout:

```sh
cargo install --path crates/sapho-cli --locked
sapho validate examples/graphs/review.yaml
sapho inspect examples/graphs/review.yaml
printf '%s' '{"support":0.7}' | sapho run examples/graphs/review.yaml --fail-on needs_review
```

The example asks for review when support is strictly below 0.8. It returns a typed Boolean and its trace. `--fail-on` makes a true selected Boolean exit 1; false exits 0. Without it, a completed run exits 0 irrespective of application decisions. Configuration, execution, acquisition and incomplete measurement failures exit 2. stdout contains JSON; process diagnostics go to stderr.

`validate` and `inspect` compile the graph and return its input/output signatures, dependency stages and required primitives/backends. They execute neither code nor models and need no credentials. The stock executable has no domain-specific Rust primitives: an unavailable `code` node refuses compilation rather than being replaced by a stub.

## Author a graph

[review.yaml](../examples/graphs/review.yaml) is a complete threshold rule:

```yaml
inputs:
  support: {kind: probability}
nodes:
  - id: review
    operation: {kind: compare, comparator: less}
    inputs:
      a: {kind: input, name: support}
      b:
        kind: literal
        value: {id: cutoff, value: {kind: probability, value: 0.8}}
        value_type: {kind: probability}
outputs:
  needs_review: {kind: node, node: review, port: result}
```

Input schemas decide how plain numbers are interpreted. Number, Probability and Degree are distinct. Literals name their schema and retain a Datum identity. Connections name an input or node output; a `path` selects declared record fields. Dependencies come from these connections and are checked before evaluation.

YAML `.yaml`/`.yml` and JSON `.json` files share one Serde schema. `--format yaml` or `--format json` explicitly selects another extension. Graph TOML is removed. Documents reject duplicate keys, merges, custom tags and expansion; zero anchor/alias budgets refuse those forms. Only `true` and `false` are implicit Booleans. Quote numeric-looking or Boolean-looking strings. Use `|` for intentional multiline question text and `|-` when a trailing newline is unwanted. The decoded question text is part of exact replay identity.

Syntax/schema/duplicates/unsupported tags or merges return `Config`; byte and parser budgets, including anchors/aliases, return `LimitExceeded`. Graph documents are capped at 1 MiB and 128 structural levels; compilation also bounds typed nesting, subgraph references and node count.

## Combine evidence and add layers

Use crisp `and`, `or`, `not` and `compare` for decisions. Explicitly convert Probability or Number to Degree before `reduce`; `min`, `max`, `weighted_mean` and `complement` express heuristic policy. A composite Degree is not a calibrated probability.

[combine.yaml](../examples/graphs/combine.yaml) maps each identified `{support, weight}` sample into a Degree and a weight, then combines them:

```sh
sapho run examples/graphs/combine.yaml --input examples/data/combine-input.json
```

The strengths 0.8, 0.4 and 0.6, with weights 2, 1 and 1, yield 0.65 and `needs_review: true`. Both maps originate from the same identified items, so their outputs retain matching IDs. Independently decoded plain lists have different occurrence IDs; use shared item records or explicit typed inputs when a weighted reducer needs alignment.

`record` assembles named inputs into context with exact field types. `list` declares homogeneous `item_type` and an explicit `order`; repeated values through different ports become distinct occurrences. For example:

```yaml
- id: strengths
  operation: {kind: list, item_type: {kind: degree}, order: [contract, tests]}
  inputs:
    contract: {kind: node, node: contract_degree, port: result}
    tests: {kind: node, node: test_degree, port: result}
```

[multilayer.yaml](../examples/graphs/multilayer.yaml) is an original two-stage example: assemble input text, ask a question, project support, assemble text plus prior support, then ask again. A later layer can also consume full Answers or identified candidate records. Use separate backend names for experts. Guards select work; map/filter/pairs/join select and combine items. Domain extraction and dynamic question construction remain registered Rust functions, supplied by your application host.

## Input, output and bounded work

`run GRAPH --input FILE` reads plain JSON input ports against the compiled schema; `--input -` reads stdin and is the default. `--typed-input` accepts existing `Inputs` and preserves caller IDs and sources. Typed inputs are the interchange produced by selectors.

Named inputs must be regular files. Unix stdin has a 30-second readiness deadline without changing shared descriptor flags; named FIFOs/devices refuse. JSON selector acquisition honors its explicit deadline and both byte ceilings.

Use `--output report.json` and `--trace trace.json` for explicit persistence. Destinations are exclusively claimed before evaluation and are never overwritten. A preparation failure after claiming destinations can leave empty newly created files; choose fresh names for another attempt. Completed runs contain outputs and trace; execution refusals contain an error and partial trace.

Defaults per evaluation are 4096 node instances, 16384 expanded items, 128 model calls, concurrency 4, 8 MiB cumulatively accounted data and 60 seconds. Flags `--max-nodes`, `--max-items`, `--max-model-calls`, `--concurrency`, `--max-data-bytes` and `--timeout-secs` make those ceilings explicit. Input defaults to 1 MiB (`--max-input-bytes`) and report/recording artifacts to 8 MiB (`--max-artifact-bytes`). A request for larger finite limits is explicit; the CLI does not crop inputs or retry to fit.

## Bind Jev explicitly

Build the optional transport support when you need it:

```sh
cargo install --path crates/sapho-cli --locked --features jev
```

Supply a bindings document separately from the graph:

```yaml
judge:
  provider: jev
  model: jev-1.13.0
  expected_model: jev-1.13.0
  distribution_policy: {kind: strict}
```

Then invoke `sapho run examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml`. The host resolves `TYPESAFE_API_KEY` before macOS Keychain/Linux Secret Service/Windows Credential Manager (scope `agent-ix/sapho`, account `jev-api-key`); the SDK receives the resolved secret. Endpoint configuration stays in the SDK. Transport debug logging is disabled in the CLI; [Jev setup](user-guide.md#connect-jev) documents it. Credential fields in graphs/bindings are refused. The default executable refuses live providers; it still supports offline logic and replay. Calls have no implicit retries.

A custom host can register a local Laya/KEV backend through `ModelBackend` and domain functions through `PrimitiveRegistry`. `sapho-cli::Runner::new` and its async `run` method accept those registries. Acquire inputs before the async call and persist the returned report afterwards. The process CLI does not load arbitrary scripts, libraries or LoRA adapters.

## Bind CLM explicitly

Use an already running host-managed [CLM service](https://github.com/Contrastive-LM/CLM); Sapho does not install or download a model.

```sh
cargo install --path crates/sapho-cli --locked --features clm
```

```yaml
judge:
  provider: clm
  model: clm-latest
  distribution_policy: {kind: strict}
```

`model` may be omitted for CLM and defaults to `clm-latest`. Jev still requires an explicit model. Set `CLM_BASE_URL` to the service base URL (default `http://127.0.0.1:8700`); Sapho appends `/v1/systemone`. HTTPS is required for remote services. Cleartext is permitted for loopback; embedded URL credentials, query strings and fragments are refused. `CLM_API_KEY` overrides the OS credential at scope `agent-ix/sapho`, account `clm-api-key`. An absent credential permits an unauthenticated local service; a locked/unavailable store is a typed refusal. Set/store credentials through your host's secret tooling, never in a graph, binding, recording or argv. No credential-file fallback exists.

```sh
sapho run examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml
sapho record examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml --recording saved.json
sapho replay examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml --recording saved.json
```

Replay works with either inferred identity or explicit CLM metadata and never reads credentials or contacts the service. Only required live bindings resolve credentials; validation/inspection and replay-backed measure/tune remain offline.

CLM calls use a 30-second queue/HTTP/decode deadline, a 1 MiB request ceiling, an 8 MiB response ceiling and four simultaneous requests per adapter. Runtime's separate limits still apply. There are no retries or redirects. Boolean criteria map to `true`/`false`; question and option order are preserved. Reported confidence is retained independently of probability: CLM defines it as top probability minus the mean of the others. Usage retains `input_tokens`, `output_tokens` and optional `billing_units` separately. Raw distributions remain unchanged under strict or approximate-complete policy.

## Bind Ollama explicitly

Use an already running [Ollama](https://ollama.com) server; Sapho never starts a server or pulls a model. The provider needs no credential and reads no secret store.

```sh
cargo install --path crates/sapho-cli --locked --features ollama
```

```yaml
judge:
  provider: ollama
  model: qwen3:30b
  distribution_policy: {kind: strict}
```

An `ollama` entry takes the members every entry has (`model`, which is required, `expected_model` and `distribution_policy`) and four of its own, with these defaults:

| Member | Default | Meaning |
|---|---:|---|
| `think` | `false` | Let the model think before it answers; typed questions need `false` |
| `num_ctx` | 32768 | Context size in tokens; a prompt that does not fit is refused, never shortened |
| `num_predict` | 512 | Tokens reserved for the answer; it must be below `num_ctx` |
| `timeout_seconds` | 600 | Bound on each single request to the server |

The default `num_predict` suits typed questions, whose answers are a few tokens; a binding that extracts records needs a larger value. The members go to the backend unchanged, so its own refusals (a zero value, an empty model, `think: true` for questions) are the ones you see, each before a request is sent. The model identity is the model name and the weights digest the server reports, and a recording carries both; a bindings entry cannot carry a digest, an `endpoint`, a `url` or an `api_key`.

Set `OLLAMA_BASE_URL` to the server (default `http://127.0.0.1:11434`; an empty value counts as unset). The URL is read from the environment only, never from a bindings document, a graph or a recording, and a URL the adapter refuses (anything but `http` or `https`, or one with embedded credentials, a query or a fragment) is refused before a request. The variable may name a non-local host. That is your choice: prompts and state then leave this machine, and a recording does not say which host answered, since it holds the model name and weights digest and never a URL. To show that a run stayed local, keep the environment with the run.

```sh
OLLAMA_BASE_URL=http://127.0.0.1:11434 sapho record examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml --recording saved.json --timeout-secs 600
sapho replay examples/graphs/multilayer.yaml --input INPUT.json --bindings bindings.yaml --recording saved.json
```

Only a required binding that calls a model live (`run`, `record`, and `measure` or `tune` without `--replay`) builds the backend. `validate`, `inspect`, `replay` and replay-backed `measure` and `tune` read no environment variable and touch no network.

Two deadlines apply. `timeout_seconds` bounds each request to the server, and the run limit `--timeout-secs` (default 60) bounds the whole evaluation of a case. Whichever comes first ends the call with the deadline error of its layer, and neither is raised implicitly. A first request that loads a large model can take longer than 60 seconds, so when the model is cold raise `--timeout-secs` to at least the binding's `timeout_seconds`. The CLI never retries a deadline.

Default tests run against a loopback double of the server. A live check against a real server runs only when you ask for it: `SAPHO_LIVE_OLLAMA=MODEL cargo test -p sapho-cli --features ollama live_server_smoke`.

OpenAI Decisions integration awaits its official preview wire contract. The current CLI offers Jev, CLM and Ollama; no substitute API is presented as Decisions.

## Record and replay

Use explicit local destinations for state/evidence containing private content:

```sh
sapho record examples/graphs/review.yaml --input INPUT.json --recording saved.json --trace trace.json
sapho replay examples/graphs/review.yaml --input INPUT.json --recording saved.json
```

That offline example saves an empty model recording. A model graph also receives `--bindings`; capture retains successful validated raw exchanges, while failures keep partial trace and available earlier exchanges. A recording is not a dataset: a model answer becomes a label only when you put it in a dataset whose provenance declares kind `model` and the model as its source.

Replay constructs only ReplayBackend: no credential lookup, transport or live fallback. Requests match exact model, policy, state and ordered question definitions. Changed combinations/thresholds can reuse an exchange when its request is unchanged. Changed prompts/context cause `ReplayMiss`. Binding metadata is inferred only when each backend has a unique `(model, expected_model, distribution_policy)` tuple. Conflicts refuse with `RecordingMismatch`; names absent from an empty/partial recording require explicit matching `--bindings` metadata. A no-model graph replays with an empty recording.

## Gather attributable data

Selectors produce typed `Inputs`; pipe them into a matching graph:

```sh
sapho select files --root ./src --include '**/*.rs' --exclude '**/generated/**' | sapho run examples/graphs/files.yaml --typed-input
sapho select git --root . --mode staged | sapho run examples/graphs/files.yaml --typed-input
sapho select git --root . --mode revisions --base BASE --head HEAD
sapho select json --input SOURCE.json --pointer /items --schema item-list.yaml --port items
```

Files and Git patches produce `List(Record{path:Text,text:Text,status:Text})`, sorted by root-relative paths. [files.yaml](../examples/graphs/files.yaml) assembles them into context without making a model call. A review tool can add domain extraction, model questions and finding formatting; per-edit scheduling and repair loops remain that tool's responsibility.

Files skip symlinks/non-regular entries and `.git`, refuse selected unreadable/non-text content, and attribute exact bytes under a content identity. Git requires 2.34+ on Unix for bounded process acquisition. `working_tree` compares tracked index/working state with HEAD; `staged` compares index with HEAD; `revisions` resolves both supplied commits. Untracked files are excluded, renames are deletion/addition, external diff/textconv/filter commands are disabled, and binary/submodule changes refuse. Paths are literal argument data. Refusal terminates and reaps owned Git work.

JSON selection uses RFC 6901: `~1` escapes `/`, `~0` escapes `~`, and empty pointer selects root. A schema file declares ValueType in YAML or JSON. It uses the shared plain-data converter and attributes the whole original document, rather than inventing precise parsed subvalue spans.

Selector defaults: 1024 selected files, 10000 visited entries, 1 MiB per file, 4 MiB total retained text, 64 KiB Git stderr and 30 seconds. `--max-files`, `--max-entries`, `--max-file-bytes`, `--max-total-bytes`, `--max-stderr-bytes` and `--timeout-secs` set positive finite limits. Empty selections are valid; incomplete/cropped context is never reported as successful.

## Curate and measure labels

A dataset is JSON with an identity and cases containing unique IDs, split `development` or `held_out`, typed `inputs`, nonempty Boolean `labels` by output, and `label_provenance`. [review-dataset.json](../examples/data/review-dataset.json) is original synthetic tutorial data, not an accuracy benchmark.

```sh
sapho measure examples/graphs/review.yaml --dataset examples/data/review-dataset.json --split development
sapho measure examples/graphs/review.yaml --dataset examples/data/review-dataset.json --split held_out
```

Boolean outputs report TP/TN/FP/FN and agreement with supplied labels. Probability outputs report Brier score against Boolean outcomes. Degree/Number, missing outputs and failed runs remain unscored/errors. Each output reports labelled, scored, unscored and failed counts with its own scored denominator; outputs are never averaged together. Empty/unscored metrics are absent. Measurements retain predictions, labels/provenance and run evidence. Exit 2 means at least one selected label was unscored or failed; the complete report is still emitted.

For model graphs, provide explicit `--bindings` for live evaluation or `--replay saved.json` for exact offline evaluation. Each case has its own declared RunLimits. Label provenance records your declaration of each label's kind (`model`, `agent`, `human` or `deterministic_check`), source and reference; Sapho does not certify that a label is true, and measure never scores a model against labels whose source is that same model.

## Evaluate a large Dataset

The case and byte ceilings are safety bounds, so their defaults stay at 1024 cases (`--max-cases`) and 8 MiB (`--max-artifact-bytes`, which also bounds the Dataset file). A Dataset of about fifteen thousand labelled cases is evaluated by raising both ceilings explicitly, so you choose the larger limit knowingly and nothing is silently cropped or retried. For a Dataset of up to 20,000 cases:

```sh
sapho measure GRAPH --dataset dataset.json --replay recording.json --split development --max-cases 20000 --max-artifact-bytes 1073741824 --output report.json
sapho tune --candidate GRAPH --dataset dataset.json --replay recording.json --output-name NAME --metric brier --max-cases 20000 --max-artifact-bytes 1073741824 --output tuning.json
```

`--max-cases` and `--max-artifact-bytes` are accepted by `measure`, `tune` and `export-training`. The artifact ceiling bounds each file separately: the Dataset file, the recording and the report are each limited to it, so choose a value (1073741824 is 1 GiB) at least as large as the biggest of them. The report holds the complete trace of every case, so it is much larger than the Dataset; the 15,000-case test Dataset of claim-sized text, a file of 29 MB, gives a report of 178 MB.

A Dataset with more cases than `--max-cases` is refused with `CaseLimit` naming the limit, before any model call, and a file larger than `--max-artifact-bytes` is refused naming the limit before it is parsed. A Dataset is one file: the CLI does not shard. The per-case result of `measure` does not depend on how cases are split into files, so a caller can split a very large Dataset and combine the per-file reports from their case counts: agreement and Brier combine as case-weighted means, and calibration bins combine by adding their counts.

The process holds the typed Dataset, the compressed per-case documents and the replay index (a recording is indexed as it is read, never held whole), so its peak memory stays near the size of the typed Dataset. Holding each case compressed costs time: the user CPU time of a very large `measure` is about 2.4 times what it was before, because each case document is compressed once and expanded three times. The test suite measures the peak of the 15,000-case run and requires at most six times the Dataset file plus 64 MiB plus 8 KiB per case, on a debug build, for Datasets of about 0.3 KB and about 1.8 KB per case.

## Tune explicit candidates

Author bounded graph variants, then compare their development results:

```sh
sapho tune --candidate examples/graphs/review.yaml --candidate examples/graphs/review-conservative.yaml --dataset examples/data/review-dataset.json --output-name needs_review --metric agreement --output tuning.json
```

The CLI keeps each candidate definition, case evidence and metrics. It ranks Boolean agreement highest or Probability Brier lowest; ties retain candidate order. Every development case must carry and score the chosen output label without failure to rank. Replay-incompatible or incomplete candidates cannot win. `--max-candidates` defaults to 16. No rankable candidate, no development cases or an exceeded ceiling is a refusal. The CLI never edits graphs, labels or recordings, and never evaluates held-out cases during tune.

Use the separate explicit held-out `measure` invocation after choosing a candidate. Changing a model request requires new captured evidence; replay never approximates a match.

## Export curated training data

```sh
sapho export-training --dataset examples/data/review-dataset.json --output development.jsonl
```

JSONL rows retain dataset/case identity, original typed inputs, supplied Boolean labels and provenance. Only development rows are emitted. Empty/unlabelled/provenance-free datasets and existing output destinations refuse. Downstream trainers own model-specific prompt/completion conversion and actual training; this command makes no model or trainer calls.

## Graph skills

[The Sapho plugin](../plugins/sapho/plugin.json) packages `create`, `tune` and `record` skills around these public commands. Creation makes input/output meaning and logic explicit, tuning uses declared labels and separated splits, and recording retains explicit local evidence for exact replay. Installing or publishing the package is a separate user action. The package follows the [portable plugin format](https://developers.openai.com/plugins/build/plugins) with root skill discovery and contains no enforcement hooks.
