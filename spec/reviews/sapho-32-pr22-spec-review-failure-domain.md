---
id: SR-055
title: "Spec failure-domain review of PR #22 (SAPHO-32)"
type: SpecReview
analysis: failure-domain
scope: "agent-ix/sapho@2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-055: spec-review/failure-domain of PR #22

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#22 at 2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12, base main f71c1e8. Closes SR-043 FND-010..014 (PR #20). Reviewer run 999c642c-bef3-444c-923d-c41e8d73df12, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Error classification of undecodable bodies at non-success statuses.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | Classifying a binary gateway error as an answer defect misleads callers that count, retry or stop by error code | spec/modules/core/functional/FR-048.md:48 |

## Analysis

### FND-001 (medium, confidence high, soundness)

Unit: FR-048 at spec/modules/core/functional/FR-048.md:48. Related: FR-051, FR-009.

> If a response body is not valid UTF-8, then the implementation SHALL return `InvalidAnswer` with reason `malformed_response` whatever the HTTP status (a binary error page from a proxy included), because the body is decoded before the status is examined and an undecodable body can be neither kept nor explained, no raw exchange (a RawExchange holds bodies as text) and usage holding the elapsed time without token counts, because the body cannot be read.

Classifying a binary gateway error as an answer defect misleads callers that count, retry or stop by error code. The stated reason is implementation order ('the body is decoded before the status is examined'), not a property callers want. `InvalidAnswer` says the model produced an unusable answer; a binary 502 or 503 from a proxy says the transport failed. Consequences: answer-quality metrics count proxy outages as malformed model output; a caller that stops on binding-level transport errors (for example a 404 `model_not_found` behind a gateway that returns a binary page) keeps going item by item; retry policies keyed on `BackendFailed` skip these. Recommendation: examine the status first; for a non-success status return `BackendFailed` (reason from the status, `model_not_found` for 404) with no raw exchange when the body is not text and usage with elapsed time; keep `malformed_response` for a success status with an undecodable body. This changes the PR #20 code (`http.rs` decodes before checking the status), so it is an owner decision; if the current behaviour is kept, say so in FR-051 and accept the trade-off explicitly.

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

0 high, 1 medium, 0 low; blocking: none.

## Dispositions

Round 1, reviewed at agent-ix/sapho@39ad473d3d505cf45508b6215007fd336bee3063.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 39ad473: Status judged before the body, with the reason stated (retry and halt rules keyed on BackendFailed keep working); the code on impl/ollama-extractor must follow (it decodes first at 558a3ce). |
