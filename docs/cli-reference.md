# CLI reference

[Documentation index](index.md) · [CLI recipes](cli-guide.md) · [Examples](examples.md)

Run commands from the checkout root after installing `sapho`, or substitute
`cargo run --locked -p sapho-cli --` for `sapho`. `--help` on any command shows
its accepted arguments. `--version` prints the version. The default executable
supports offline logic, selection, measurement and replay; live Jev/CLM calls
require the corresponding Cargo feature.

## Commands

| Command | Required arguments | Optional command-specific arguments | Result |
|---|---|---|---|
| `validate GRAPH` | Graph path | `--format yaml\|json` | The same Inspection JSON as `inspect`; exit 2 if the graph does not compile; no evaluation |
| `inspect GRAPH` | Graph path | `--format yaml\|json` | JSON signature, stages, primitives and backend names |
| `run GRAPH` | Graph path | Run options below | JSON outputs and trace, or failure with partial trace |
| `record GRAPH` | Graph path, `--recording PATH` | Same run options | Run report and successful model exchanges |
| `replay GRAPH` | Graph path, `--recording PATH` | Same run options | Exact offline run report |
| `measure GRAPH` | Graph path, `--dataset PATH`, `--split development\|held_out` | `--format`, measurement options below | Coverage, per-output metrics and case predictions |
| `tune` | Repeated `--candidate GRAPH`, `--dataset PATH`, `--output-name NAME`, `--metric agreement\|brier` | `--format`, `--max-candidates 16`, measurement options | Development measurements and ranked candidates |
| `export-training` | `--dataset PATH`, `--output PATH` | `--max-cases 1024`, `--max-artifact-bytes 8388608` | Development supervision JSONL |
| `select files` | `--root DIR` | Path and acquisition options below | Typed Inputs of file records |
| `select git` | `--root DIR`, `--mode working_tree\|staged\|revisions` | `--base REF`, `--head REF`, path/acquisition options | Typed Inputs of patch records |
| `select json` | `--input PATH`, `--schema PATH` | `--pointer ''`, `--format yaml\|json`, `--id-prefix selected`, acquisition options | Schema-directed typed Inputs |

The examples page supplies complete commands for each command. Custom `code`
nodes require a Rust host; validate/inspect cannot substitute implementations.

## Run and measurement options

Run/record/replay accept:

| Option | Default | Meaning |
|---|---|---|
| `--format yaml\|json` | Path extension | Graph encoding; does not change input JSON |
| `--input PATH` | `-` | Plain JSON named inputs; `-` reads stdin |
| `--typed-input` | False | Input is serialized `Inputs` with Datum IDs/sources |
| `--bindings PATH` | Absent | YAML/JSON backend metadata by name |
| `--output PATH` | Stdout | Persist complete JSON report |
| `--trace PATH` | Absent | Persist trace separately |
| `--fail-on NAME` | Absent | Exit 1 if this named Boolean output is true |

`--fail-on` requires a declared Boolean output; Optional(Boolean) and Degree
are not finding ports. Without it, true outputs still produce exit 0.

Measure/tune accept `--bindings`, `--replay PATH`, `--output`, `--max-cases`
(default 1024), and all evaluation limits. They consume dataset case inputs,
so they have no `--input`, `--typed-input`, `--trace` or `--fail-on` flags.
`--replay` uses recorded exchanges without a live provider. Tune always uses
development cases; measure requires an explicit split. Agreement maximizes
Boolean agreement; Brier minimizes squared Probability error against Boolean
labels. Degree outputs are unsupported for scoring. A candidate must score
the selected output for every development case to rank. Ties retain candidate
order. Training export performs no inference and no training.

## Evaluation limits

Run, record, replay, measure and tune accept these positive ceilings. Runtime
limits apply per case evaluation and are shared across nested maps.

| Option | Default | Resource |
|---|---:|---|
| `--max-nodes` | 4096 | Executed node instances |
| `--max-items` | 16384 | Cumulatively expanded collection items |
| `--max-model-calls` | 128 | Explicit model requests |
| `--concurrency` | 4 | Model calls in flight at once; other work runs one node at a time |
| `--max-data-bytes` | 8388608 | Cumulatively accounted serialized inputs/outputs |
| `--timeout-secs` | 60 | Monotonic evaluation duration |
| `--max-input-bytes` | 1048576 | The `--input` document of run/record/replay |
| `--max-artifact-bytes` | 8388608 | Datasets, `--recording`/`--replay` files read, and report, trace and recording artifacts written |

The CLI refuses over-budget work and does not crop input. Validate/inspect
use graph loader/compiler limits and do not accept these evaluation flags.

## Selection options

Files and Git accept repeatable `--include GLOB` (default `**/*`) and
`--exclude GLOB` (default empty). Patterns are root-relative with `/`
separators; `.git` directories are skipped. Files are sorted regular UTF-8
text files; symlinks are not followed. Selection returns complete content,
not truncated excerpts.

Git requires Unix and Git 2.34+. `working_tree` compares tracked working/index
state with HEAD; `staged` compares the index with HEAD; `revisions` requires
both `--base` and `--head` commits. Base/head are invalid for the other modes.
Untracked files are not included. Binary patches and submodule changes refuse.
The explicit root must be the repository root.

JSON selection reads a regular file, projects an RFC 6901 pointer, then checks
against an explicit ValueType schema. `--pointer ''` selects the whole document;
`/records` selects an object member; `~0` escapes `~`, `~1` escapes `/`.
There is no inferred schema. Schema format follows its extension or `--format`.
`--id-prefix` controls selected occurrence identities. Source references identify
the whole document, not inferred byte spans.

All three selectors accept:

| Option | Default | Meaning |
|---|---:|---|
| `--port` | `items` | Named output input port |
| `--output PATH` | Stdout | Persist typed Inputs |
| `--max-files` | 1024 | Selected files/patches |
| `--max-entries` | 10000 | Visited directory entries |
| `--max-file-bytes` | 1048576 | One file or process stdout |
| `--max-total-bytes` | 4194304 | Retained content across selection |
| `--max-stderr-bytes` | 65536 | Git diagnostics |
| `--timeout-secs` | 30 | Total acquisition duration |
| `--max-artifact-bytes` | 8388608 | Serialized Inputs artifact |

JSON uses its applicable byte/time limits; it does not walk files. Pipe selector
stdout into `run --typed-input` with a graph whose input schema matches.

## Backend bindings and credentials

A bindings document maps graph backend IDs to metadata:

```yaml
judge:
  provider: clm
  model: clm-latest
  expected_model: null
  distribution_policy: {kind: strict}
```

`provider` is `jev`, `clm`, `systemone` or `ollama`. Jev, System One and Ollama require an explicit
nonempty `model`; CLM may omit it and defaults to `clm-latest`. An `ollama`
entry also takes `think` (false), `num_ctx` (32768), `num_predict` (512) and
`timeout_seconds` (600); its server URL comes from `OLLAMA_BASE_URL` (default
`http://127.0.0.1:11434`, a non-local host allowed) and it needs no credential
([guide](cli-guide.md#bind-ollama-explicitly)). `expected_model` optionally requires
an exact actual response identity. `distribution_policy` is required: strict,
or `{kind: approximate, max_mass_error: 0.02}` with a positive allowance at
most 0.05. Graphs/bindings do not contain credentials or endpoints.

Live Jev resolves `TYPESAFE_API_KEY`, then the OS secret store account
`jev-api-key` in scope `agent-ix/sapho`. CLM uses `CLM_API_KEY`, then account
`clm-api-key`; no secret permits an unauthenticated service, but invalid or
unavailable credential storage can refuse. Native lookup is synchronous and
only performed for required live providers. `CLM_BASE_URL` defaults to
`http://127.0.0.1:8700`; the adapter appends `/v1/systemone`. Remote services
require HTTPS. Endpoint/auth details and Rust construction are in the
[API reference](api-reference.md#model-adapters).

For `systemone`, live commands select a bounded JSON host file using `--service-config FILE` or `SAPHO_SERVICE_CONFIG` (path only). Its `services` object maps exact backend IDs to `base_url`, optional OS-store `credential_key`, and optional complete `limits` (`timeout_ms`, `request_bytes`, `response_bytes`, `in_flight`). Limits cannot exceed 30000 ms, 1 MiB, 8 MiB and 4. The host loads only required services, never for replay; see [the guide](cli-guide.md#bind-multiple-system-one-services).

Replay infers binding identity from recorded requests; explicit metadata must
agree. An empty/partial recording may require metadata for missing backend
identities. Changed model, policy, state or ordered questions prevents an exact
match. Changed downstream thresholds can reuse requests. Replay has no live
fallback or credential lookup.

## Reports, persistence and exit status

Reports and selectors emit JSON to stdout; diagnostics go to stderr. A run
report has four fields: `outputs`, a map of typed Datums; `trace`, the execution
evidence; `error`, a structured refusal or `null`; and `exit`, which is
`completed`, `finding` or `refused`. When evaluation refuses, `outputs` is
`null`, `trace` holds the work done before the refusal and `exit` is `refused`.
A `--fail-on` output that cannot be selected keeps `outputs` and reports its
error with `exit: refused`. Use `value.to_plain_json()` in Rust or inspect Datum
`value` fields when consuming typed results. Trace/model evidence can retain
your input text.

Named output, trace, recording and export files are exclusively created and
never overwritten. Choose fresh paths. Preparation can leave newly claimed
empty files after a later refusal. Persist acquisition and recordings outside
Tokio workers in an embedding host.

Exit 0 means completed, 1 means a true explicitly selected finding, and 2 means
configuration/execution/coverage refusal. Measurement with unscored or failed
labels refuses even when it emits a useful report. Argument parsing errors also
exit 2. Input files must be regular files; Unix stdin has a 30-second readiness
budget. See [structured errors](api-reference.md#execution-traces-and-errors)
for programmatic classification.
