---
id: SR-048
title: "Spec failure-domain review of PR #21 (SAPHO-32)"
type: SpecReview
analysis: failure-domain
scope: "agent-ix/sapho@ebab022c10835bb091a7b82a60b8073cfb75aa36; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-048: spec-review/failure-domain of PR #21

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#21 at ebab022c10835bb091a7b82a60b8073cfb75aa36, base main 9876a4a. Relates to SR-043 (PR #20 gap analysis). Reviewer run 02df86a0-ffb3-4ca1-ba77-83e247418fd5, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Credential exposure and size of raw exchanges in recordings and traces.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | RawExchange 'exact request bytes sent' does not exclude HTTP headers, so an authenticated backend could put credentials into recordings | spec/modules/core/functional/FR-048.md:32 |

## Analysis

### FND-001 (medium, confidence medium, coverage)

Unit: FR-048 at spec/modules/core/functional/FR-048.md:32. Related: FR-006, NFR-003, FR-044.

> `ExtractResponse.raw`: the exact request bytes sent and the exact response bytes received, so a caller can store them by content hash.

RawExchange 'exact request bytes sent' does not exclude HTTP headers, so an authenticated backend could put credentials into recordings. Ollama sends no credentials and the PR #20 code keeps body bytes only, but FR-006 now extends the RawExchange to every ModelBackend. CLM sends a bearer token and Jev an SDK credential in headers; 'exact request bytes sent' admits the full HTTP request, which would put secrets into ModelResponse, recordings and traces, against NFR-003 ('recordings declare no API-key, auth-header ... field'). Define the raw exchange as the request and response body bytes only, never headers or URLs.

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

0 high, 1 medium, 0 low; blocking: none.

