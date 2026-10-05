---
id: SR-044
title: "Spec spec-review review of PR #21 (SAPHO-32)"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@ebab022c10835bb091a7b82a60b8073cfb75aa36; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-044: spec-review of PR #21

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#21 at ebab022c10835bb091a7b82a60b8073cfb75aa36, base main 9876a4a. Relates to SR-043 (PR #20 gap analysis). Reviewer run 02df86a0-ffb3-4ca1-ba77-83e247418fd5, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Umbrella spec review of PR #21 (spec amendment for SR-043 plus public-repo leak sweep), ticket SAPHO-32.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | Leak sweep left a ticket fragment and merged two consumers' requirement numbers | spec/reviews/sapho-75-spec-review.md:94 |

## Analysis

### FND-001 (low, confidence high, other)

Unit: spec/reviews/sapho-75-spec-review.md at spec/reviews/sapho-75-spec-review.md:94. Related: SR-033.

> Non-blocking but due before the milestone that builds them: the Ollama prompt-cache effect on prompt_eval_count (sapho SR-038 FND-001, verify on the local inference host before SAPHO-32), same-weights aliases counted as independent sources (downstream dataset consumer SR-006 FND-002, before a downstream ticket), the all-true Boolean claim export (downstream dataset consumer SR-009 FND-001, before a downstream ticket/<number>), and the open items below.

Leak sweep left a ticket fragment and merged two consumers' requirement numbers. The replacement of a private ticket pair left 'a downstream ticket/<number>' (line 94), which still exposes a private ticket number and reads as garbled. The replacement of one consumer's FR-004, FR-008 and the other consumer's FR-007, FR-009 with four times 'the downstream dataset consumer's FR-…' (SR-033 FND-001 related IDs) merges two repositories, so FR-007/FR-009 now point at the wrong document. Use 'downstream tickets' and 'the downstream consumers' FR-004, FR-008 (pipeline) and FR-007, FR-009 (question stages)'.

## Scope

| Unit | Path | Role |
|------|------|------|
| SR-026 | reviews/26-10-02-providers-code-review.md | examined |
| SR-027 | reviews/26-10-02-providers-gap-analysis.md | examined |
| SR-028 | reviews/26-10-03-public-readiness-code-review.md | examined |
| FR-005 | spec/modules/core/functional/FR-005-validate-model-answers.md | examined |
| FR-006 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| TC-006 | spec/modules/core/test_cases/TC-006.md | examined |
| FR-049 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-054 | spec/modules/ollama/functional/FR-054.md | examined |
| TC-049 | spec/modules/ollama/test_cases/TC-049.md | examined |
| TC-054 | spec/modules/ollama/test_cases/TC-054.md | examined |
| SR-039 | spec/reviews/sapho-75-spec-review-criterion-strength.md | examined |
| SR-037 | spec/reviews/sapho-75-spec-review-dependency.md | examined |
| SR-038 | spec/reviews/sapho-75-spec-review-failure-domain.md | examined |
| SR-034 | spec/reviews/sapho-75-spec-review-integrity.md | examined |
| SR-036 | spec/reviews/sapho-75-spec-review-scope-boundary.md | examined |
| SR-033 | spec/reviews/sapho-75-spec-review.md | examined |
| FR-035 | spec/modules/cli/functional/FR-035.md | context_only |
| FR-044 | spec/modules/clm/functional/FR-044.md | context_only |
| FR-027 | spec/modules/recording/functional/FR-027-record-successful-backend-exchanges.md | context_only |
| NFR-003 | spec/modules/core/non-functional/NFR-003.md | context_only |

## Verdict

0 high, 0 medium, 1 low; blocking: none.

## SR-043 closure check

| SR-043 finding | Closed by | Judgement |
|----|----|----|
| FND-001 raw exchange on the question path | FR-006 new statement, FR-006-AC-4/5, FR-052 bullet, FR-054 raw bullet, FR-054-AC-9 | Closed in substance: ModelResponse and SaphoError now carry the raw exchange and AC-4/5/9 can fail. The amendment contradicts FR-035 and FR-044 as written (SR-045 FND-001 and FND-002). |
| FND-005 raw-retention SHALL without AC or carrier | FR-006 carrier, FR-054-AC-9 | Closed; AC-9's wording needs the request too (SR-047 FND-002). |
| FND-006 FR-048-AC-6 method | Verification now `Test` | Closed. |
| FND-007 attribution before value start / past value end | Two FR-054 bullets and FR-054-AC-10 | Rules stated; AC-10's first example cannot fail for the new rule (SR-047 FND-001). |
| FND-008 FR-049 vs FR-048 parsing | FR-049 now returns bytes and core parses; AC-3 reworded | Closed. |
| (FND-004 in SR-043) request_model_differs without a criterion | FR-049 bullet and FR-049-AC-6 | Closed; AC-6 can fail (zero requests recorded). |

## Public-repo leak sweep

Own `git grep` over the whole tree at ebab022 for the private repository, host, plan and project names, private ticket IDs and pipeline stage names (triage, annotate, harvest, quotas, claims/confirm stage, first-author): no hits outside the swept text below.

Word diff of the eight swept review artifacts against main: every change replaces a name (host, private repository, plan and ticket names, model and stage names) with a neutral phrase; no other text changed. Meaning is kept, with two slips recorded as SR-044 FND-001.

Left on purpose, judged:
- EARS mentions in spec.md, tests.md, IT-002, the SR-035 "ears analysis" title and older reviews: legitimately public; EARS is a public requirement notation and these describe a synthetic consumer shape.
- Public ticket IDs (SAPHO-1/3/4/5/16/32/57/75): fine; IDs carry no content.
- `qwen3:30b`, the `-opencode` tags, the weights-blob digest prefix and Ollama 0.32.14: public model and server facts; fine.
- docs/yaml-authoring-assessment.md names the private repository quire-semantic, a commit of it and measurements of its graph, and names the owner; 26-09-30-rounded-distribution-boundary.md mentions saved private EARS development runs. Neither leaks data, and both predate this PR; whether a public repo should name a private repo is an owner call (not a finding here, outside the diff).
- Git history still holds the pre-sweep text (the squash commit 9876a4a on main and the spec/ollama-extractor branch commits). Rewriting public history is an owner decision, not ours.

## Merged M1 spec regression check

`quire validate --summary` exits 0 with 195/195 documents grammar-clean; IDs are unique; every ix:// target and relative link resolves (the SR-033 excerpt link is fixed). The new criteria are untagged, as expected for a spec-only PR whose code is PR #20.

## Bundle Verdict

Not mergeable: SR-045 FND-001 (FR-006 contradicts FR-035's "raw HTTP bodies ... are not new fields") blocks. Everything else is non-blocking.

## New findings (disposition pass 1)

Reviewed at agent-ix/sapho@809dc3bab03c5d8e6d488f92da0f7bfeb669891b.

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-002 | medium | BLOCKER (public repo): sapho-75-spec-review.md:98 still names the retired private campaign tool | spec/reviews/sapho-75-spec-review.md:98 |

### FND-002 (medium, confidence high, other, blocking)

Unit: spec/reviews/sapho-75-spec-review.md at spec/reviews/sapho-75-spec-review.md:98. Related: SR-044 FND-001.

> Projection v1, Python, <private campaign tool name redacted>), no task-log wording, and no Sapho reference to the downstream dataset consumer.

BLOCKER (public repo): sapho-75-spec-review.md:98 still names the retired private campaign tool. The list of deprecated items in the bundle-validation paragraph ends with the retired private campaign tool's own name; the sweep's grep list did not include it. Widened whole-repo grep at 809dc3b (`campaign`, `ears`, `ears-`, `dataset consumer`, `downstream`) finds no other private name: every other `EARS`/`ears` hit is the public requirement grammar (spec.md:23/106, IT-002, the 26-09-30 review, the `ears-conformance` analysis type, quire's `ears:non-singular` check name), and `downstream` hits are generic. Names-only fix: replace the tool name with 'the retired campaign tool' or drop it from the list. (This review redacts the name in its own excerpt, so the artifact does not repeat the leak.)


## Dispositions

Round 1, reviewed at agent-ix/sapho@809dc3bab03c5d8e6d488f92da0f7bfeb669891b.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 809dc3b: The 'ticket/<number>' fragment is gone ('the tickets that build it'), and the merged attribution is split into the consumers' FR-004, FR-008 (pipeline) and FR-007, FR-009 (question stages). Word diff shows names and grammar only; meanings kept. |
