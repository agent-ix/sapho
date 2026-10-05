---
id: SR-051
title: "Spec spec-review review of PR #22 (SAPHO-32)"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-051: spec-review of PR #22

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#22 at 2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12, base main f71c1e8. Closes SR-043 FND-010..014 (PR #20). Reviewer run 999c642c-bef3-444c-923d-c41e8d73df12, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Umbrella spec review of sapho PR #22, which aligns spec wording with the implemented behaviour for SR-043 FND-010..014, ticket SAPHO-32.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | The amended FR-048 non-UTF-8 sentence no longer parses | spec/modules/core/functional/FR-048.md:48 |

## Analysis

### FND-001 (low, confidence high, other)

Unit: FR-048 at spec/modules/core/functional/FR-048.md:48. Related: FR-048-AC-7.

> If a response body is not valid UTF-8, then the implementation SHALL return `InvalidAnswer` with reason `malformed_response` whatever the HTTP status (a binary error page from a proxy included), because the body is decoded before the status is examined and an undecodable body can be neither kept nor explained, no raw exchange (a RawExchange holds bodies as text) and usage holding the elapsed time without token counts, because the body cannot be read.

The amended FR-048 non-UTF-8 sentence no longer parses. The bullet now reads '... `malformed_response` whatever the HTTP status (...), because the body is decoded before the status is examined and an undecodable body can be neither kept nor explained, no raw exchange (...) and usage holding the elapsed time without token counts, because the body cannot be read.' The list of what the error carries is detached from its verb and there are two `because` clauses. Rewrite as: 'the implementation SHALL return `InvalidAnswer` with reason `malformed_response`, no raw exchange and usage holding the elapsed time only, whatever the HTTP status; ...' (or as SR-055 FND-001 recommends).

## Scope

| Unit | Path | Role |
|------|------|------|
| FR-035 | spec/modules/cli/functional/FR-035.md | examined |
| FR-046 | spec/modules/cli/functional/FR-046.md | examined |
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-054 | spec/modules/ollama/functional/FR-054.md | examined |
| TC-051 | spec/modules/ollama/test_cases/TC-051.md | examined |
| TC-054 | spec/modules/ollama/test_cases/TC-054.md | examined |
| FR-027 | spec/modules/recording/functional/FR-027-record-successful-backend-exchanges.md | examined |
| TC-027 | spec/modules/recording/test_cases/TC-027.md | examined |
| SR-043 | spec/reviews/sapho-32-gap-analysis.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | context_only |
| SR-043 | spec/reviews/sapho-32-gap-analysis.md | context_only |

## Verdict

0 high, 0 medium, 1 low; blocking: none.

## SR-043 FND-010..014 closure

| SR-043 finding | Closed by | Judgement |
|----|----|----|
| FND-010 question-path request_model_differs | FR-054 bullet and FR-054-AC-11 | Closed; the AC can fail (zero requests recorded). |
| FND-011 CLI error object members | FR-046 text and FR-046-AC-3: exactly `code`, `message`, `context`, reason in `context.reason` | Closed. |
| FND-012 recording conflict check and replayed raw | FR-027 text, FR-027-AC-6, FR-035-AC-4 | Closed; matches `ReplayBackend::new` on `impl/ollama-extractor` (558a3ce): duplicates compare with `raw: None`, and the first exchange is kept (`insert` only when absent). |
| FND-013 non-UTF-8 body at any status | FR-048, FR-048-AC-7, FR-052 | Stated, but now contradicts FR-051's status mapping (SR-052 FND-001) and the chosen behaviour is a trap for callers (SR-055 FND-001). |
| FND-014 `Server::with_header` | FR-051 Inputs bullet and FR-051-AC-5 | Closed; the implementation marks the value sensitive and keeps headers out of the raw exchange. |

SR-043 is committed as the reviewer's latest copy (byte-identical).

## Public-repo leak check

Python-regex grep of the added diff lines and of spec/reviews/sapho-32-gap-analysis.md for private repository, host, plan and campaign names, the private consumer and evaluation repositories, quire-semantic and private ticket IDs: no hits. This review's own files were checked the same way.

## Bundle Verdict

Not mergeable: SR-052 FND-001 (FR-048 and FR-051 give a binary non-success response two different error codes) blocks; a one-sentence change to either requirement clears it, and SR-055 FND-001 recommends which.

