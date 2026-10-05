---
id: SR-052
title: "Spec integrity review of PR #22 (SAPHO-32)"
type: SpecReview
analysis: integrity
scope: "agent-ix/sapho@2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-052: spec-review/integrity of PR #22

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#22 at 2b7d6a50bb4a5cd4b41d37d59d46d827f9c02d12, base main f71c1e8. Closes SR-043 FND-010..014 (PR #20). Reviewer run 999c642c-bef3-444c-923d-c41e8d73df12, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Consistency of the amended FR-027, FR-035, FR-046, FR-048, FR-051, FR-052 and FR-054 with each other and with the implementation on impl/ollama-extractor.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | BLOCKER: FR-048 now makes a non-UTF-8 non-success response `InvalidAnswer`, while FR-051 maps every non-success status to `BackendFailed` | spec/modules/core/functional/FR-048.md:48 |

## Analysis

### FND-001 (high, confidence high, soundness, blocking)

Unit: FR-048 at spec/modules/core/functional/FR-048.md:48. Related: FR-051, FR-051-AC-4, FR-052.

> If a response body is not valid UTF-8, then the implementation SHALL return `InvalidAnswer` with reason `malformed_response` whatever the HTTP status (a binary error page from a proxy included), because the body is decoded before the status is examined and an undecodable body can be neither kept nor explained, no raw exchange (a RawExchange holds bodies as text) and usage holding the elapsed time without token counts, because the body cannot be read.

BLOCKER: FR-048 now makes a non-UTF-8 non-success response `InvalidAnswer`, while FR-051 maps every non-success status to `BackendFailed`. FR-051 (unchanged, line 40): 'The adapter SHALL map HTTP 404 to `BackendFailed` with reason `model_not_found`, and other non-success statuses and connection failures to `BackendFailed`.' FR-048 now says a body that is not UTF-8 gives `InvalidAnswer`/`malformed_response` 'whatever the HTTP status', and FR-048-AC-7 tests a 502. A binary 502 or 404 therefore has two required outcomes. Either add the exception to FR-051 (and FR-051-AC-4) or, better, keep FR-051 and make FR-048's rule apply to success statuses only (SR-055 FND-001).

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

1 high, 0 medium, 0 low; blocking: FND-001.

