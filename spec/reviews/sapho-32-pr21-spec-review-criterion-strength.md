---
id: SR-047
title: "Spec criterion-strength review of PR #21 (SAPHO-32)"
type: SpecReview
analysis: criterion-strength
scope: "agent-ix/sapho@ebab022c10835bb091a7b82a60b8073cfb75aa36; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-047: spec-review/criterion-strength of PR #21

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#21 at ebab022c10835bb091a7b82a60b8073cfb75aa36, base main 9876a4a. Relates to SR-043 (PR #20 gap analysis). Reviewer run 02df86a0-ffb3-4ca1-ba77-83e247418fd5, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Whether the new criteria FR-006-AC-4/5, FR-048-AC-6, FR-049-AC-3/6 and FR-054-AC-8/9/10 can fail (manual reading; Jev not used).

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | FR-054-AC-10's first example cannot fail when the new before-value-start rule is removed | spec/modules/ollama/functional/FR-054.md:71 |
| FND-002 | low | FR-054-AC-9 recomputes 'from the raw response bytes .. | spec/modules/ollama/functional/FR-054.md:70 |

## Analysis

### FND-001 (medium, confidence high, untestable-ac)

Unit: FR-054-AC-10 at spec/modules/ollama/functional/FR-054.md:71. Related: FR-054.

> With the generated token `yes` starting exactly at the value start, a listed alternative ` "no` (whose bytes before the value start, ` "`, differ from the generated token's empty prefix) is not attributed; a listed alternative `no"` that runs past the value end is not attributed, and `no` then takes the bound `b`.

FR-054-AC-10's first example cannot fail when the new before-value-start rule is removed. With the generated `yes` starting exactly at the value start, the value starts at offset 0 of the position, so ` "no` has no bytes before the value start under the offset reading; it is rejected only because ` "no` is not a prefix of `no`, which the older rule already did. The AC's stated split (bytes before the value start ` "`) assumes a different alignment than the bullet, and either way a mutant that drops the new rule passes. Use a case the rule alone decides: generated token `"y` (bytes before the value start `"`) and listed alternative ` n` (bytes before the value start ` `, then `n`, a prefix of `no`): it must not be attributed. The `no"` half is fine.

### FND-002 (low, confidence high, ambiguous)

Unit: FR-054-AC-9 at spec/modules/ollama/functional/FR-054.md:70. Related: FR-054.

> Recomputing every answer's probabilities from the raw response bytes in the returned ModelResponse alone, with the rules above, gives exactly the reported probabilities; a `logprobs_mismatch` failure is a SaphoError that carries the raw request and response bytes and the usage.

FR-054-AC-9 recomputes 'from the raw response bytes ... alone', but attribution also needs the allowed values from the request. The response holds the generated JSON and the logprobs; the allowed values, their order and the question types are in the request's `format` schema. Say 'from the raw exchange (request and response) alone'.

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
| FR-006-AC-1 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-006-AC-2 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-006-AC-3 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-006-AC-4 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-006-AC-5 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | examined |
| FR-048-AC-1 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-2 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-3 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-4 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-5 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-6 | spec/modules/core/functional/FR-048.md | examined |
| FR-049-AC-1 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-2 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-3 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-4 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-5 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-6 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-054-AC-1 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-2 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-3 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-4 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-5 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-6 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-7 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-8 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-9 | spec/modules/ollama/functional/FR-054.md | examined |
| FR-054-AC-10 | spec/modules/ollama/functional/FR-054.md | examined |

## Verdict

0 high, 1 medium, 1 low; blocking: none.

