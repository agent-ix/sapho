---
id: SR-045
title: "Spec integrity review of PR #21 (SAPHO-32)"
type: SpecReview
analysis: integrity
scope: "agent-ix/sapho@ebab022c10835bb091a7b82a60b8073cfb75aa36; spec amendment and review-artifact sweep (origin/main...HEAD); ticket SAPHO-32"
review_set: subset
---
# SR-045: spec-review/integrity of PR #21

## Summary

Ticket: SAPHO-32. PR: agent-ix/sapho#21 at ebab022c10835bb091a7b82a60b8073cfb75aa36, base main 9876a4a. Relates to SR-043 (PR #20 gap analysis). Reviewer run 02df86a0-ffb3-4ca1-ba77-83e247418fd5, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735.

Consistency of the raw-exchange amendment with existing core, recording, CLI and CLM requirements.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | BLOCKER: FR-006 puts raw HTTP bodies into ModelResponse, which recordings and traces keep, contradicting FR-035's 'raw HTTP bodies .. | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:40 |
| FND-002 | medium | Raw exchange on every SaphoError conflicts with FR-044 and leaves CLI error output undefined | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:40 |

## Analysis

### FND-001 (high, confidence high, soundness, blocking)

Unit: FR-006 at spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:40. Related: FR-035, FR-027, FR-017, FR-054-AC-9.

> A ModelResponse SHALL carry an optional raw exchange, the exact request bytes sent and response bytes received, using the same RawExchange type as extraction ([FR-048](FR-048.md)); it is omitted from JSON when absent, so recordings made without it still load. When a backend failure happens after an exchange took place, the SaphoError SHALL keep that raw exchange and the reported usage; the conversion of an ExtractError into a SaphoError carries both. A probability or a failure can then be audited against the bytes that produced it.

BLOCKER: FR-006 puts raw HTTP bodies into ModelResponse, which recordings and traces keep, contradicting FR-035's 'raw HTTP bodies ... are not new fields'. FR-027's RecordingBackend records the raw core response and FR-017's ask evidence keeps the raw normalized response, so ModelResponse.raw now lands in every recording and trace, while FR-035 (still on main) says 'raw HTTP bodies and credentials are not new fields'. FR-006-AC-4 even tests loading a recording without the member, so the conflict is intended but unrecorded. Scenario: an implementer following FR-035 strips raw from recordings, and FR-054-AC-9's audit fails after replay; one following FR-006 grows recordings past FR-027's caller-supplied byte ceiling with 20-alternative logprob bodies. Amend FR-035 (and say in FR-027/FR-017 that the raw exchange is retained and counts toward the byte ceiling), or state that recordings and traces drop it.

### FND-002 (medium, confidence high, ambiguous)

Unit: FR-006 at spec/modules/core/functional/FR-006-define-replaceable-model-backend.md:40. Related: FR-044, FR-034, FR-045, NFR-003.

> A ModelResponse SHALL carry an optional raw exchange, the exact request bytes sent and response bytes received, using the same RawExchange type as extraction ([FR-048](FR-048.md)); it is omitted from JSON when absent, so recordings made without it still load. When a backend failure happens after an exchange took place, the SaphoError SHALL keep that raw exchange and the reported usage; the conversion of an ExtractError into a SaphoError carries both. A probability or a failure can then be audited against the bytes that produced it.

Raw exchange on every SaphoError conflicts with FR-044 and leaves CLI error output undefined. The statement applies to any backend, but FR-044 (CLM) says errors never include response bodies or request content. The intent ('bytes travel in a typed field, never in the message') is FR-051's wording for Ollama, not FR-044's. The CLI emits structured errors as machine JSON (FR-034) and promises private contents are excluded from diagnostics; nothing says whether a serialized SaphoError carries the raw field. Scope the rule ('an adapter that retains the raw exchange SHALL keep it on the SaphoError'), reword FR-044 to 'messages never include', and state that the CLI omits the raw field from printed errors.

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

1 high, 1 medium, 0 low; blocking: FND-001.

