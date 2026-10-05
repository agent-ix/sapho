---
id: SR-043
title: "Gap analysis of the Ollama extractor (SAPHO-32)"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho@f88f1a29347750b5311c03ae340d2dfb9aacc528; FR-048, FR-049, FR-050, FR-051, FR-052, FR-053, FR-054, FR-036, FR-005 with TC-048..TC-054, TC-036, IT-007; tests in crates/sapho-core, crates/sapho-ollama, crates/sapho-evidence, crates/sapho-cli; source in the PR diff against origin/spec/ollama-extractor; planless; ticket SAPHO-32 (also SAPHO-57)"
review_set: subset
relationships:
  - { target: "ix://agent-ix/sapho/FR-054", type: references }
  - { target: "ix://agent-ix/sapho/FR-048", type: references }
  - { target: "ix://agent-ix/sapho/IT-007", type: references }
---
# SR-043: Gap analysis of the Ollama extractor

## Summary

Ticket: SAPHO-32 (also SAPHO-57). PR: agent-ix/sapho#20 at f88f1a29347750b5311c03ae340d2dfb9aacc528. Method: gap-analysis, planless (reviewer run 95098AEB-F4A4-4928-A2A5-5A198A1358CD, claude-opus-5-5, quoin 0.28.1, quire 0.36.1 engine 0.50.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05. Plan completion: not assessed.

`quire matrix --strict` exits 0. Every in-scope criterion is `tagged` except FR-048-AC-6, which is `method-without-symbol`. Each binder was read against its criterion, and each SHALL in the in-scope FRs was traced to code.

## Verdict

FAIL: one blocking finding. FR-054 requires the question path to keep the raw request and response bytes so that each probability can be recomputed. Nothing on that path keeps them, and no criterion tests it, which is why the matrix is green. All other SHALL statements are implemented and tested. The remaining findings are trace-tag errors and four spec defects.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | high | The question path keeps no raw exchange; FR-054 probabilities cannot be recomputed | crates/sapho-ollama/src/questions.rs:379 |
| FND-002 | medium | The schema-violation test is tagged FR-050-AC-3 and IT-007-SC-04 but checks IT-007-SC-02 | crates/sapho-ollama/tests/extraction.rs:628 |
| FND-003 | low | The weights-mismatch test is tagged IT-007-SC-02; IT-007-SC-03 and SC-04 have no tagged test | crates/sapho-ollama/tests/extraction.rs:560 |
| FND-004 | low | The request_model_differs test is tagged FR-049-AC-1; no criterion covers that refusal | crates/sapho-ollama/tests/extraction.rs:614 |
| FND-005 | low | Spec: FR-054's raw-retention SHALL has no acceptance criterion and the port has no carrier | spec/modules/ollama/functional/FR-054.md:54 |
| FND-006 | low | Spec: FR-048-AC-6 declares Inspection although a mechanical test verifies it | spec/modules/core/functional/FR-048.md:60 |
| FND-007 | low | Spec: FR-054 attribution does not cover bytes before the value start or past its end | spec/modules/ollama/functional/FR-054.md:49 |
| FND-008 | low | Spec: FR-049 says the adapter parses the answer, but FR-048 has core parse returned bytes | spec/modules/ollama/functional/FR-049.md:37 |

## Analysis

### FND-001 (high, confidence high, soundness) — blocks merge

FR-054: "The adapter SHALL keep the raw request and response bytes, including every listed alternative, so each probability can be recomputed from the record." FR-052 also requires the raw bytes "on every completed exchange, on success and inside every error raised after a response arrived". On the question path, `ModelBackend::infer` returns `ModelResponse { model, digest, answers, usage }` (crates/sapho-core/src/model.rs:425-435), which has no raw field. Every error goes through the `from` closure at questions.rs:379: `e.with_raw(generated.raw.clone()).into()`. `From<ExtractError> for SaphoError` (crates/sapho-core/src/extract.rs:158-181) copies only the reason, pointer and TooLarge numbers, so the `with_raw` is discarded and looks like retention when it is not. Scenario: a run records a Boolean answer at probability 0.97, and the logprobs and the 20 alternatives that produced it exist nowhere, so the number cannot be audited. A `logprobs_mismatch` failure likewise loses the bytes that would explain it. Extraction keeps raw bytes correctly. Fixing this needs a place to carry the bytes through the ModelBackend port: for example, an optional raw field on `ModelResponse` that recordings store, and raw bytes in SaphoError context or a typed error detail. That is a change to the FR-006 port contract, so the owner must decide between a spec cycle (FR-006/FR-054 plus a criterion) and a scoped-out amendment of FR-054 before the fix round.

### FND-002 (medium, confidence high, trace)

`schema_violation_from_the_server_keeps_pointer_and_exchange` (extraction.rs:628-640) asserts `InvalidAnswer`/`schema_violation` with pointer `/label` and the raw exchange and usage. That is IT-007-SC-02 ("InvalidAnswer with the violation pointer and the raw exchange") and FR-048-AC-2 through a real adapter. It is tagged FR-050-AC-3 (TooLarge takes precedence over truncation) and IT-007-SC-04 (TooLarge with raw and usage), and it asserts neither. Fix: retag it as IT-007-SC-02.

### FND-003 (low, confidence high, trace)

`changed_weights_and_a_different_model_name_are_mismatches` (extraction.rs:560-589) is FR-052-AC-4, correctly tagged, but it also carries IT-007-SC-02, a scenario about schema violation. IT-007-SC-03 (the server's context refusal gives TooLarge with its count) is checked by extraction.rs:165-194, and SC-04 (TooLarge from an oversized prompt_eval_count, with raw and usage) by extraction.rs:225-249, but neither test carries the IT tag. Since `quire matrix` does not list IT scenarios, nothing flags this. Fix: drop IT-007-SC-02 from line 560, and add IT-007-SC-03 at line 165 and IT-007-SC-04 at line 225.

### FND-004 (low, confidence medium, trace)

`a_request_for_another_model_than_the_binding_is_refused` (extraction.rs:614-626) checks the `request_model_differs` Config refusal, which no FR-049 criterion mentions. Its FR-049-AC-1 tag says it verifies the request member set, which it does not. Fix: drop the tag, and add the refusal to FR-049's behavior and criteria if it is to be traced.

### FND-005 (low, confidence high, coverage) — spec defect

FR-054 line 54 states the raw-retention SHALL, but none of FR-054-AC-1..8 tests it, and the ModelBackend port (FR-006) defines no carrier for raw bytes. This gap let FND-001 pass a strict matrix. Fix: add a criterion (for example "the ModelResponse or its error retains the exact request and response bytes, including every listed alternative") together with the port change, or drop the bullet.

### FND-006 (low, confidence high, other) — spec defect

FR-048-AC-6 declares verification `Inspection`, and quire reports it as `method-without-symbol`, the same status as FR-047-AC-1/2 and NFR-001-M-1. The criterion is checked mechanically by `core_manifest_has_no_transport_runtime_and_validator_resolves_nothing` (crates/sapho-core/tests/extract.rs:206-235), and `cargo tree -p sapho-core` confirms no runtime, HTTP or file crate. The tag is correct; the declared method is the defect. Fix: change the method to `Test`, or keep `Inspection` and bind it to the symbol kind that method expects. `--strict` exits 0 either way, so this does not block.

### FND-007 (low, confidence medium, ambiguous) — spec defect

FR-054 attributes a candidate when "the candidate's bytes from there on are a nonempty prefix of that value". It does not say whether a candidate whose bytes before the value start differ from the generated token's stands at the same position. The code requires them to match (coder decision d). It also does not cover a candidate that runs past the value end (for example `no"` containing the closing quote), which is never attributed. That value then falls back to the bound b, which can be lower than its real listed mass. Fix: state both rules in FR-054 and add a criterion for each.

### FND-008 (low, confidence high, ambiguous) — spec defect

FR-049 says the adapter "SHALL parse its `response` text as JSON and return it as the answer value" and return `not_json` itself. FR-048 makes core parse the returned bytes ("returned bytes that are not JSON produce InvalidAnswer ... not_json"). The code follows FR-048: `Completion.answer` is bytes, and `extract` parses and validates them (coder decision a). Fix: change FR-049's bullet to say the adapter returns the response text and core parses it.

### Clean units

FR-048-AC-1..5: the port, the single checked path, the refusals before calling the implementation, and `$ref` safety. A `$ref` that leaves the document, including `file://` and URL forms, is refused before compiling; jsonschema has no resolve features. Tests are adequate. FR-049-AC-1..5, FR-050-AC-1..5, FR-051-AC-1..4, FR-052-AC-1..5, FR-053-AC-1..4: implemented and tested; mutants M1, M2, M5, M7, M15 and M19 were killed. Test-strength gaps are in SR-042. FR-054-AC-1..8: implemented; the AC-2 vector gives 0.9715, and counting the generated token twice (M4) turns five tests red. FR-036-AC-1..5: provenance kinds, blank source or reference refused as MissingProvenance, self_source excluded by name or digest including failed cases, and the CLI's exit status unaffected. FR-005-AC-1: the digest is preserved and stays optional.

### Outside this diff

This diff contains no out-of-scope content. Out-of-scope wording in files from the base spec PR was reported to the team leader for that PR.

## Scope

| Unit | Path | Role |
|---|---|---|
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| FR-049 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-050 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-053 | spec/modules/ollama/functional/FR-053.md | examined |
| FR-054 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-036 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-005 | spec/modules/core/functional/FR-005-validate-model-answers.md | examined |
| IT-007 | spec/modules/ollama/integration/IT-007.md | examined |
| TC-048 | spec/modules/core/test_cases/TC-048.md | examined |
| FR-006 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | context_only |

## New findings (disposition pass 1)

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-009 | low | Spec: FR-054-AC-10's example does not exercise the pre-value rule it names, and its binder does not test it | spec/modules/ollama/functional/FR-054.md |
| FND-010 | low | Spec: FR-054 does not state the question path's request_model_differs refusal, and its test is untagged | crates/sapho-ollama/tests/questions.rs:563 |

### FND-009 (low, confidence high, trace)

FR-054-AC-10 says a listed alternative ` "no`, next to a generated `yes` that starts exactly at the value start, is not attributed because "its bytes before the value start, ` "`, differ from the generated token's empty prefix". The answer position is offset 0 inside the generated token, so under the rule the alternative's bytes before the value start are also empty. It is excluded because its remaining bytes ` "no` are not a prefix of `no`, not by the pre-value rule. Its binder, candidates_with_other_prefix_bytes_or_running_past_the_value_end_are_not_attributed (questions.rs:552), still passes with the pre-value rule deleted (mutant M6). The rule is actually tested by an_alternative_with_different_bytes_before_the_value_is_not_attributed, which is tagged FR-054-AC-2. Fix: reword AC-10's example to a generated ` "yes` with a listed `:"no`, and tag that test FR-054-AC-10.

### FND-010 (low, confidence high, coverage)

`ModelBackend::infer` refuses a ModelRequest whose model differs from the binding's with `Config`/`request_model_differs`, and a_request_for_another_model_than_the_binding_is_refused_before_sending (questions.rs:563) tests that, but FR-054 does not state the refusal, and the test carries no Trace tag. FR-049-AC-6 covers only ExtractRequest on the Extractor port, so the question path is not covered by it. Judgment: a small spec gap, not acceptable as covered. Fix: add one sentence and a criterion to FR-054 (or widen FR-049-AC-6 to both ports), then tag the test.

## Dispositions

Round 1, reviewed at ce6b41f035492e7ef94d63fb2b8ed6fa7936ccaa (diff origin/spec/model-raw-and-sweep...HEAD; spec fixes at base ebab022c10835bb091a7b82a60b8073cfb75aa36). `quire matrix --strict` exits 0 with every in-scope criterion `tagged`, including FR-006-AC-4/5, FR-048-AC-6, FR-049-AC-6 and FR-054-AC-9/10.

| FND | Outcome | sha/reason |
|---|---|---|
| FND-001 | fixed | ea98149: ModelResponse.raw and SaphoError.raw/usage; the From conversion keeps raw and usage; the question path returns raw: Some(generated.raw) and attaches raw and usage to every error; questions.rs:510 recomputes the Boolean probability from the retained bytes alone, and a logprobs_mismatch error carries the response bytes and usage; mutants M22, M23 and M24 fail |
| FND-002 | fixed | ea98149: the schema-violation test is tagged FR-048-AC-2, IT-007-SC-02 |
| FND-003 | fixed | ea98149: IT-007-SC-02 removed from the weights test; IT-007-SC-03 added at extraction.rs:165 and IT-007-SC-04 at extraction.rs:225 |
| FND-004 | fixed | ce6b41f: the test is tagged FR-049-AC-6, which spec base ebab022 adds for request_model_differs |
| FND-005 | fixed | ebab022 (base spec PR #21): FR-054-AC-9 and FR-006-AC-4/5 plus the raw carrier in FR-006, all bound to tests |
| FND-006 | fixed | ebab022 (base spec PR #21): FR-048-AC-6 verification changed to Test; matrix status now tagged |
| FND-007 | fixed | ebab022 (base spec PR #21): FR-054 states the pre-value and past-the-end rules and adds FR-054-AC-10 (see new FND-009 on its example) |
| FND-008 | fixed | ebab022 (base spec PR #21): FR-049 now says the adapter returns the response bytes and the core parses them |

## New findings (disposition pass 2)

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-011 | low | Spec: FR-046-AC-3 lists reason as a member of the CLI error object; reason lives in context | spec/modules/cli/functional/FR-046.md |
| FND-012 | low | Spec: FR-027 does not say the duplicate-request conflict check ignores the raw exchange, or which raw replay returns | crates/sapho-recording/src/lib.rs:197 |
| FND-013 | low | Spec: a non-UTF-8 body is malformed_response at every HTTP status, so a binary 5xx becomes an answer defect | crates/sapho-ollama/src/http.rs:187 |
| FND-014 | low | Spec: Server::with_header is a public configuration input that FR-051 does not list | crates/sapho-ollama/src/http.rs:101 |

### FND-011 (low, confidence high, ambiguous)

FR-046-AC-3 says the error object's "members are exactly code, reason, message and context fields". The FR-046 statement says it holds "the error code, its reason when present, its content-free message and its named context fields". The SaphoError JSON contract has carried reason as `context["reason"]` since before this PR, and the From<ExtractError> conversion follows it. The CLI writes `{code, message, context}`, and invocation.rs:388 asserts exactly that set. The behaviour is right and the evidence is excluded, but the criterion reads as if `reason` were a top-level member. Ruling on decision (a): not blocking. Amend FR-046 and AC-3 to say "code, message and context, with the reason as the context field `reason`".

### FND-012 (low, confidence high, coverage)

ReplayBackend's "Conflicting response for identical request" check (recording/src/lib.rs:197-212) now compares responses with `raw` cleared. Two live calls with an identical request always differ in `created_at` and duration counters, so the old check would refuse every re-recorded duplicate. This is tested by identical_requests_whose_raw_bodies_differ_in_timing_are_not_a_conflict, and mutant N5 is killed. FR-027 does not state it. Replay keys exchanges by request, so both duplicates replay the first exchange's raw bodies. FR-035-AC-4's "same raw exchange byte for byte" therefore holds only for the first. Ruling on decision (b): the behaviour is correct and not blocking. Amend FR-027 to say the conflict check compares everything but the raw exchange, and that replay returns the first recorded raw exchange for a repeated request.

### FND-013 (low, confidence medium, ambiguous)

`Server::post` decodes the body as UTF-8 before any status handling (http.rs:187-200). A non-UTF-8 body therefore yields `InvalidAnswer`/`malformed_response` at any status: a 502 HTML-with-binary page from a proxy, a non-UTF-8 404, and a non-UTF-8 `/api/show` reply (which would otherwise be `BackendFailed`/`http_status` or `digest_unavailable`). This matches FR-048 and FR-052 as amended ("If a response body is not valid UTF-8 ..."), so it is not a code defect. But it classifies a transport failure as an answer defect, which a caller retrying or counting failures by code would misread. Judgment: not blocking. Either accept it as written, or amend FR-048 so a non-success status takes precedence (`BackendFailed`/`http_status`, usage with elapsed time, no raw) and only a 2xx non-UTF-8 body is `malformed_response`.

### FND-014 (low, confidence high, coverage)

`Server::with_header(name, value)` (http.rs:101-115) is new public API. It adds a header to every request, marks the value sensitive (so Server's derived Debug prints `Sensitive`), and never copies it into a raw exchange: the header-injecting double tests run through it and mutant N1 is killed. The spec names only a test transport ("the header-injecting loopback double ... whose test transport adds the header"). FR-051's configuration list (base URL, timeout, ceilings) does not include request headers, so a production feature for authenticating gateways is unspecified. Judgment: keep it; it is the smallest honest way to drive the double through the real client, and it is useful. Not blocking; add it to FR-051's inputs (headers sent with every request, values never in a raw exchange, error, Debug or log). The root crate's new dev-dependency on sapho-ollama is test-only (tests/scenarios/raw.rs), changes no shipped dependency graph, and needs no amendment.

## Dispositions (round 2)

Round 2, reviewed at 558a3ce7c12350fc48fc77c0f68cf7fa945df788 (diff origin/main...HEAD, main f71c1e8 with spec PRs #19 and #21 merged). `quire matrix --strict` exits 0; the only non-tagged rows are the earlier FR-047-AC-1/2 and NFR-001-M-1.

| FND | Outcome | sha/reason |
|---|---|---|
| FND-009 | fixed | 558a3ce (test) on spec f71c1e8: FR-054-AC-10 is now the `"y` against ` n` fixture (0.9970 with the rule, 0.8581 without); a_candidate_not_sharing_the_bytes_before_the_value_is_not_attributed (questions.rs:552) asserts 0.9970 and fails under mutant M6; the past-the-end clause is gone from FR-054 |
| FND-010 | deferred | Small spec amendment batched by the team leader into one sapho spec PR: add the question path's request_model_differs refusal to FR-054 (or widen FR-049-AC-6 to both ports), then tag questions.rs:567. The behaviour is implemented and tested; not blocking |
