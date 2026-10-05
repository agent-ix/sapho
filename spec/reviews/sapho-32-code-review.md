---
id: SR-042
title: "Code and Rust review of the Ollama extractor (SAPHO-32)"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho@f88f1a29347750b5311c03ae340d2dfb9aacc528; diff origin/spec/ollama-extractor...HEAD: crates/sapho-ollama/**, crates/sapho-core/src/extract.rs, error.rs, model.rs, lib.rs, tests/extract.rs, src/tests.rs, crates/sapho-evidence/src/lib.rs and tests.rs, crates/sapho-cli/src/command.rs and tests, Cargo.toml, Cargo.lock, deny.toml, docs/*, examples/data/*, CLAUDE.md; ticket SAPHO-32 (also SAPHO-57)"
review_set: subset
---
# SR-042: Code and Rust review of the Ollama extractor

## Summary

Ticket: SAPHO-32 (also SAPHO-57). PR: agent-ix/sapho#20 at f88f1a29347750b5311c03ae340d2dfb9aacc528, stacked on spec/ollama-extractor (c6b1abc0ac647fc22f0340402671397c77138385). Method: code-review with the rust-review lane folded in (reviewer run 95098AEB-F4A4-4928-A2A5-5A198A1358CD, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

The test-oracle strength of every new loopback test was checked by mutating the code under test and running `cargo test -p sapho-ollama --tests` once per mutant.

## Verdict

PASS WITH FINDINGS at the code level. The adapter is a correct, bounded, stateless implementation: exact request members, `truncate: false`/`shift: false`, nested context refusal parsed into `TooLarge`, one process-wide permit across extraction, questions and embeddings held over `/api/show` and generate, single attempt with redirects off and no proxy, request and response ceilings, per-binding digest pin, and FR-054 probabilities that match the AC-2 vector without double counting. No panic path, `unsafe`, blocking call or lock across `.await` was found. Two medium findings are test gaps where a regression of a stated property would pass the suite. The one blocking defect for this PR (FR-054 raw bytes are not kept on the question path) is recorded in the gap analysis SR-043 FND-001, not here.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | medium | No test catches /api/show running outside the request permit | crates/sapho-ollama/src/backend.rs:201 |
| FND-002 | medium | No test catches a response ceiling that trusts Content-Length | crates/sapho-ollama/src/http.rs:146 |
| FND-003 | low | The selected-mass half of the bound b is untested | crates/sapho-ollama/src/questions.rs:261 |
| FND-004 | low | The shared pre-value bytes attribution rule is untested and goes beyond FR-054 | crates/sapho-ollama/src/questions.rs:249 |
| FND-005 | low | The prompt_eval_count boundary (above vs at num_ctx) is untested | crates/sapho-ollama/src/backend.rs:267 |
| FND-006 | low | has_foreign_ref refuses a schema whose data or property name is "$ref" | crates/sapho-core/src/extract.rs:255 |
| FND-007 | low | Allowed::new strips an escaped trailing quote from a label | crates/sapho-ollama/src/questions.rs:34 |

## Analysis

### FND-001 (medium, confidence high, test-intent)

`OllamaBackend::generate` (backend.rs:201-209) and `OllamaEmbedder::embed` (embed.rs:105-111) resolve the digest inside `Server::exclusive`, so the permit covers `/api/show` and the generate or embed call. Mutant M3 moved `identity::resolve` out of `exclusive` (show unserialized, generate still serialized): every test passed, including `concurrent_callers_never_overlap_and_a_cancelled_waiter_frees_the_way` (extraction.rs:308) and `embedding_and_extraction_share_the_one_request_permit` (embedding.rs:117). The fake's `/api/show` replies instantly, so a show that overlaps another caller's generate is never observed. Scenario: a later refactor moves the show call out of the permit; a second binding's `/api/show` lands while a 30B generate is running, which makes the server load the other model and swap, and the change ships green. Fix: delay the fake's `/api/show` reply (as the generate reply is delayed) so `max_active == 1` also guards the show request.

### FND-002 (medium, confidence high, test-intent)

FR-051 requires the response ceiling to hold "independent of Content-Length". `Server::post` (http.rs:146-156) counts streamed chunks, which is correct. Mutant M9 replaced that with a check of `response.content_length()` and an unbounded read: every test passed. The only oversize test (extraction.rs:451-469) sends an honest Content-Length larger than the ceiling. Scenario: a regression to a header check lets a chunked or under-declared 2 GB reply be read into memory. Fix: add a fake reply with a chunked body or a Content-Length smaller than the body sent, and assert `LimitExceeded`.

### FND-003 (low, confidence high, test-intent)

`masses` (questions.rs:261-265) sets `bound = lowest.min(selected_mass)` as FR-054 states. Mutant M12 (`bound = lowest`) survived: in every test the selected token's probability exceeds the lowest listed alternative, and no successful test has an empty `top_logprobs`, where `lowest` is infinite and the bound must fall back to the selected mass. Scenario: a regression drops the `min`, and a response whose alternatives list is empty makes every question fail with `logprobs_mismatch` ("Probabilities are not usable"), with no test going red. Fix: add a Boolean case with an empty alternatives list, where the expected probability is 0.5, and one where the generated token is below every listed alternative.

### FND-004 (low, confidence high, test-intent)

questions.rs:249-253 skips a listed alternative unless its bytes before the value start equal the generated token's (coder decision d). FR-054 attributes a candidate by its bytes "from there on" and says nothing about the bytes before. The rule is conservative: a candidate with a different structural prefix (`:"no` against ` "yes`) is not counted. But it is beyond the spec text, and mutant M6 (rule deleted) survived. The only multi-byte-prefix test (questions.rs:418) uses alternatives that share the prefix. Fix: add a test with an alternative whose pre-value bytes differ, and record the rule in FR-054 (see SR-043 FND-007).

### FND-005 (low, confidence high, test-intent)

backend.rs:267-269 raises `TooLarge` when `prompt_eval_count + num_predict > num_ctx` ("above", per FR-050). Mutant M20 (`>=`) survived, because the AC-3 numbers (900 + 200 against 1000) are far from the boundary. Scenario: an off-by-one regression refuses prompts that fill the context exactly and the suite stays green. Fix: add a case at exactly `num_ctx` (expect success) and one at `num_ctx + 1` (expect `TooLarge`).

### FND-006 (low, confidence high, code-bug)

`has_foreign_ref` (extract.rs:255-267) treats every object key named `$ref` or `$dynamicRef` as a reference, at any depth, including where the key is a property name (`{"properties": {"$ref": {"type": "string"}}}`) or data inside `const`, `enum`, `default` or `examples`. The value there is not a string, so the check refuses it as `schema_external_ref`. Scenario: a host extracting JSON Schema or OpenAPI fragments, whose records have a `$ref` member, cannot declare that member. Fix: only treat `$ref` as a reference where it is a keyword (in a schema object, not directly under `properties`, `$defs`, `patternProperties` or inside data keywords), or document the restriction.

### FND-007 (low, confidence medium, code-bug)

`Allowed::new` (questions.rs:34-38) computes the written form as `serde_json::to_string(value).trim_matches('"')`. For a label ending in a double quote, the encoded form is `"a\""`, and `trim_matches` removes both trailing quote characters, leaving `a\`. The first byte is still right, so distinctness and first-token attribution hold, but a candidate covering the whole value is not recognised as a prefix. Fix: strip exactly one leading and one trailing quote (`s[1..s.len()-1]`).

### Rust-review lane

- Panic surface: no `unwrap`/`expect`/indexing that can fail in non-test code; `by_value[index]` and `allowed[selected]` index with positions from the same vectors; `locate` uses checked addition.
- Async and locks: the permit is a `tokio::sync::Semaphore` acquired inside the timed future, so a cancelled or timed-out call releases it; no `std::sync::Mutex` is held across `.await`; `OnceLock` pins the digest without blocking.
- Integer conversions at the wire: counts and durations are `u64` end to end; `temperature` and `top_logprobs` are `u8` constants; no `as` casts on wire values.
- Resource bounds: request ceiling checked before the permit; response read in chunks against the ceiling; JSON depth: a probe with 200,000 nested arrays in `response`, and in an ignored envelope member, returned `InvalidAnswer` with no stack overflow (serde_json keeps its default recursion limit; `unbounded_depth` only adds an opt-in method).
- Dependencies: reqwest has `default-features = false` with `rustls-tls` and `stream` (no gzip, brotli or other compressed encodings); jsonschema has `default-features = false` (no HTTP or file resolution); `cargo tree -p sapho-core` shows no runtime, HTTP or file-access crate.
- No deprecated items: no byte-as-token estimate, no pre-send size refusal other than the byte ceiling, no preflight certificates, no compressed encodings, nothing held between calls except the spec-mandated digest pin. No mention of private datasets or the campaign in this diff.

### Coder decisions

(a) `Extractor::exchange` returns bytes and core parses and validates: correct, it makes FR-048's single checked path hold for every implementation. (b) `request_model_differs`: sound. (c) The system-text layout matches FR-054 and is pinned by questions.rs:144-150. (d) Pre-value byte rule: conservative, see FND-004. (e) Reasons: a closed set; `request_unserializable` is unreachable in practice but harmless. (f) `/api/show` only: matches FR-052. (g) Permit and timeout over show and generate: right, and it removes a model swap between the two requests; untested, see FND-001. (h) `.no_proxy()`: keeps loopback traffic away from system proxies; a remote server reachable only through a proxy cannot be used, and that is documented on `Server::new`. (i) deny.toml: the `sapho-ollama` exception is justified and minimal (the same pattern as every first-party crate). `MIT-0` is needed by `borrow-or-share` 0.2.4 (pulled in by jsonschema) and is justified, because MIT-0 is MIT without the attribution clause, but it is not minimal: a crate-scoped `[[licenses.exceptions]] name = "borrow-or-share", allow = ["MIT-0"]` would be. `cargo deny check licenses` exits 0. (j) No nesting guard is needed (probe above). (k) See SR-043 FND-006.

## Validation

- `cargo test -p sapho-ollama -p sapho-core -p sapho-evidence` in a fresh clone at f88f1a2: exit 0 (log rv20-test.log).
- Mutation run (rv20-mut/run.sh, results.txt): killed M1 truncate:true, M2 lock dropped, M4 generated token counted twice, M5 extra request member, M7 truncation checked before TooLarge, M15 redirects followed, M16 distinctness check removed, M17 token-bytes check removed, M18 missing logprobs treated as empty, M19 digest pin removed; survived M3, M6, M9, M12, M20 (FND-001..005).
- `cargo deny check licenses`: exit 0.
- The coder's gate log ends `head=f88f1a29347750b5311c03ae340d2dfb9aacc528 exit=0`, so the full gate was not re-run.

## Scope

| Unit | Path | Role |
|---|---|---|
| crates/sapho-ollama/src/backend.rs | crates/sapho-ollama/src/backend.rs | examined |
| crates/sapho-ollama/src/http.rs | crates/sapho-ollama/src/http.rs | examined |
| crates/sapho-ollama/src/identity.rs | crates/sapho-ollama/src/identity.rs | examined |
| crates/sapho-ollama/src/embed.rs | crates/sapho-ollama/src/embed.rs | examined |
| crates/sapho-ollama/src/questions.rs | crates/sapho-ollama/src/questions.rs | examined |
| crates/sapho-ollama/tests/** | crates/sapho-ollama/tests | examined |
| crates/sapho-core/src/extract.rs | crates/sapho-core/src/extract.rs | examined |
| crates/sapho-core/tests/extract.rs | crates/sapho-core/tests/extract.rs | examined |
| crates/sapho-evidence/src/lib.rs | crates/sapho-evidence/src/lib.rs | examined |
| crates/sapho-cli/src/command.rs | crates/sapho-cli/src/command.rs | examined |
| deny.toml | deny.toml | examined |
| Cargo.toml | Cargo.toml | examined |
| docs/api-reference.md | docs/api-reference.md | examined |
