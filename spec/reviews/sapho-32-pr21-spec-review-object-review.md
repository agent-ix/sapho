---
id: SR-050
title: "Spec object-review review of PR #21 (SAPHO-32)"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@ebab022c10835bb091a7b82a60b8073cfb75aa36; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-050: spec-review/object-review of PR #21

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#21 at ebab022c10835bb091a7b82a60b8073cfb75aa36, base main 9876a4a. Relates to SR-043 (PR #20 gap analysis). Reviewer run 02df86a0-ffb3-4ca1-ba77-83e247418fd5, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

RawExchange, ModelResponse and SaphoError object definitions.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | The JSON form of RawExchange bytes is unspecified | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:49 |

## Analysis

### FND-001 (low, confidence medium, other)

Unit: FR-006-AC-4 at spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:49. Related: FR-048, FR-027.

> A ModelResponse with a raw exchange round-trips through JSON with its bytes unchanged; one without it serializes with no `raw` member, and a recording written without the member loads.

The JSON form of RawExchange bytes is unspecified. FR-006-AC-4 requires a byte-exact JSON round trip but no encoding. The PR #20 type serializes `Vec<u8>` as an array of integers, about four characters per byte, so a logprob-heavy response of 60 KB becomes about 240 KB in every recording and trace. State the encoding (base64, or a UTF-8 string with a base64 fallback) so recordings stay readable and bounded.

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

## Dispositions

Round 1, reviewed at agent-ix/sapho@809dc3bab03c5d8e6d488f92da0f7bfeb669891b.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 809dc3b: JSON form fixed as UTF-8 strings `{request, response}`; non-UTF-8 response refused as malformed_response (see new LOW SR-048 FND-002). |
