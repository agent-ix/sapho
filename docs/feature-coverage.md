# Documentation feature coverage

[Documentation index](index.md)

This inventory follows the exported Rust API families, graph vocabulary and
CLI command surface. Each row has a user reference and an executable example.
`make docs-check` runs the examples and CLI recipes. The table itself is
maintained by hand. For the graph vocabulary, the
[reference runner](../examples/reference.rs) maps every operation, comparator,
reducer and question kind to the reference case that runs it; the mapping has
no wildcard arm, so a new variant does not compile until it names a case, and
the runner checks that the case's graph uses it. Individual public symbols have
generated rustdoc, and each leaf crate has a compiled crate-level usage example.

| Feature | Reference | Runnable example/check |
|---|---|---|
| Typed IDs, Probability/Degree, values, schemas, Datum, sources, ports, strict JSON | [Values](api-reference.md#values-and-interchange) | core doctest; [reference host](../examples/reference.rs); [acquisition](../crates/sapho-select/examples/acquisition.rs) |
| YAML/JSON, formats, bindings, projection, compiler, stages, reusable definitions | [Graph](graph-reference.md#document-and-bindings) | [facts](../examples/reference/facts.yaml); graph doctest; all dual-encoding cases |
| `record`, `list`, `and`, `or`, `not` | [Ports](graph-reference.md#operation-ports) | [facts](../examples/reference/facts.yaml) |
| `compare`: `equal`, `less`, `less_equal`, `greater`, `greater_equal` | [Logic](graph-reference.md#logic-and-degrees) | [facts](../examples/reference/facts.yaml); CLI threshold boundaries |
| `degree`, `reduce`: `min`, `max`, `weighted_mean`; `complement` | [Degrees](graph-reference.md#logic-and-degrees) | [strengths](../examples/reference/strengths.yaml); empty reduction |
| `map`, captures, `filter`, `pairs`, `join`, `collect` | [Collections](graph-reference.md#collections-and-reusable-graphs) | [collections](../examples/reference/collections.yaml); [model map](../examples/reference/model-collection.yaml); item budget refusal |
| Guards, Optional, `coalesce`, skip model work | [Guards](graph-reference.md#guards-absence-and-failure) | [guards](../examples/reference/guards.yaml); [guarded model](../examples/reference/guarded-model.yaml) |
| `code`, Primitive, PrimitiveRegistry, params, cancellation context | [Compilation](api-reference.md#graph-parsing-and-compilation) | [native graph](../examples/reference/native.yaml) and TextLength in [host](../examples/reference.rs) |
| `questions`, `ask`, `probability`; question kinds `boolean`, `choice`, `score` | [Questions](graph-reference.md#questions-and-inference) | [questions](../examples/reference/questions.yaml); model layers and collection recipe |
| Answers, ModelRequest/Response, Usage, response validation, `strict`/`approximate`, complete/partial/unavailable distribution | [Answer semantics](api-reference.md#questions-answers-and-distributions) | distributions in [host](../examples/reference.rs); provider doctests |
| ModelBackend, BackendBinding/Registry, model expectation | [Execution](api-reference.md#execution-traces-and-errors) | TutorialBackend and registration in [host](../examples/reference.rs) |
| Runtime limits, concurrency, nested work, traces/status, structured failures | [Execution](api-reference.md#execution-traces-and-errors) | runtime doctest; guarded/collection runs and node statuses in [host](../examples/reference.rs); runtime deadline tests |
| RecordingBackend, Exchange/Recording, snapshot, serialization, persistence, ReplayBackend | [Recording](api-reference.md#recording-and-replay) | every reference case captures/replays; [CLI recipe](examples.md#record-replay-measure-and-tune); exclusive-write refusal |
| File patterns, selection limits, attribution, JSON pointer/schema, Git modes `working_tree`, `staged`, `revisions` | [Selection](api-reference.md#selection-apis) | [acquisition](../crates/sapho-select/examples/acquisition.rs); CLI checks use an isolated Git fixture |
| Dataset/Case/Split, Boolean labels/provenance, outcomes, coverage, agreement/Brier, ranking, training export | [Evidence](api-reference.md#evidence-apis) | [measurement](../crates/sapho-evidence/examples/measurement.rs); evidence doctest; CLI recipe |
| Provider `jev`, JevBackend, SDK client, no retries, errors | [Adapters](api-reference.md#model-adapters) | [Jev configure](../crates/sapho-jev/examples/jev_configure.rs), compiled only; [live CLI setup](cli-guide.md#bind-jev-explicitly) |
| Provider `clm`, ClmBackend, Limits, Transport, endpoint/auth, confidence/usage | [Adapters](api-reference.md#model-adapters) | [CLM configure](../crates/sapho-clm/examples/clm_configure.rs), runs offline without inference; [live CLI setup](cli-guide.md#bind-clm-explicitly) |
| Provider `ollama`, `think`/`num_ctx`/`num_predict`/`timeout_seconds`, `OLLAMA_BASE_URL`, two deadlines | [Ollama binding](cli-guide.md#bind-ollama-explicitly) | CLI loopback-double tests in [ollama.rs](../crates/sapho-cli/tests/ollama.rs); live check on demand |
| Extractor, ExtractRequest/Response/Error, `extract`, ScriptedExtractor, `TooLarge`; `sapho-ollama` OllamaBackend, OllamaEmbedder, Server, Limits | [Adapters](api-reference.md#model-adapters) | core `extract` and `sapho-ollama` loopback tests; [live check](../crates/sapho-ollama/examples/live_smoke.rs) run on demand |
| System One codec, Boolean/choice/score conversion, source-free state | [Codec](api-reference.md#model-adapters) | systemone doctest; adapter tests |
| CLI host Runner/Inspection/RunReport/ExitStatus, format/I/O/artifacts, bindings and credential resolution | [Embedding](api-reference.md#cli-embedding-apis) | CLI host doctest; process recipes; host credential tests |
| Commands `validate`, `inspect`, `run`, `record`, `replay`, `measure`, `fit-calibration`, `tune`, `export-training`, `select`; selectors `files`, `git`, `json` | [CLI reference](cli-reference.md) | [recipes](examples.md); calibration CLI fixture; `sapho COMMAND --help` |
| All CLI options/defaults, plain/typed input, limits, artifacts and exits | [CLI reference](cli-reference.md) | CLI recipe checks; `sapho COMMAND --help` |
| Plugin create/tune/record workflows | [Plugin usage](../README.md#use-the-skills-in-claude-code) | [create skill](../plugins/sapho/skills/create/SKILL.md), [tune skill](../plugins/sapho/skills/tune/SKILL.md), [record skill](../plugins/sapho/skills/record/SKILL.md); underlying CLI recipes |

Live provider inference and loading the Claude plugin require external services
or a Claude host and are outside the offline checks. Their setup is documented;
the Jev construction example is compiled and the CLM one runs offline. The
default and all-feature test suites cover core semantic behavior.
