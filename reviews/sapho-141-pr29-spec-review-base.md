---
id: SR-067
title: "Base review of corrective Ollama identity contract"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@b155ab198cd836c86b170319ca8eb2311e6bb5a7; FR-052, FR-036, FR-055, FR-054, FR-005, FR-048, FR-053, US-012, StR-012, IT-007, ollama/spec.md, TC-055"
review_set: subset
---
# SR-067: Corrective Ollama identity contract base review

## Summary

Ticket: SAPHO-141. The edited Ollama and measurement contracts remove answer-weight digest requirements, but linked unedited artifacts still require one. The Quoin Hash / Digest / Pin Antipattern applies at high severity: a `/api/show` observation cannot bind an inference response to exact answering weights.

## Verdict

FAIL. The remaining normative chain contradicts the corrective contract and the runtime, which returns no digest.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | FR-005 behavior still requires an optional digest of actual answering weights | spec/modules/core/functional/FR-005-validate-model-answers.md:29 |
| FND-002 | high | FR-048 still promises a weights digest on ExtractResponse | spec/modules/core/functional/FR-048.md:33 |
| FND-003 | high | FR-053 output and AC-1 still require an embedding model digest | spec/modules/ollama/functional/FR-053.md:27 |
| FND-004 | high | IT-007-SC-01 still requires the removed digest and contradicts its tagged test | spec/modules/ollama/integration/IT-007.md:40 |
| FND-005 | high | US-012 acceptance example still promises a model digest | spec/modules/ollama/usecase/US-012.md:19 |
| FND-006 | high | StR-012 rationale still uses a model digest to promise label traceability | spec/modules/ollama/stakeholder/StR-012.md:29 |
| FND-007 | high | Ollama module scope still requires a digest on every response | spec/modules/ollama/spec.md:18 |
| FND-008 | high | TC-055 expected results still require recorded digests | spec/modules/cli/test_cases/TC-055.md:29 |

## Coverage

- Examined FR-052 AC-1 through AC-7, FR-036 AC-4 and AC-5, FR-055 AC-3, FR-054 AC-7, FR-005 AC-1, their changed behavior text, and the Ollama source attribution procedure. These edited units avoid a required digest and explicitly limit the model-name claim.
- Examined the linked FR-048, FR-053 AC-1, IT-007-SC-01, US-012, StR-012, Ollama module scope and TC-055 expected results. Each remaining digest requirement is listed above.
- `quire validate --scope . 'spec/**/*.md'` exited 0. The structural pass does not detect the semantic contradictions.

## New findings (disposition pass 1)

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-009 | high | FR-048 still instructs content-hash storage without a canonical proof-binding use | spec/modules/core/functional/FR-048.md:32 |
| FND-010 | high | StR-012 still promises every exchange is attributable to the exact model despite the name-only reply | spec/modules/ollama/stakeholder/StR-012.md:23 |

## Dispositions

| FND | outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-002 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-003 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-004 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-005 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-006 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-007 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-008 | fixed | 0f5cdc077dedcd97012eb6f11dd801c5b263569d |
| FND-009 | fixed | 0f5cdc077dedcd97012eb6f11dd801c5b263569d |
| FND-010 | fixed | 0f5cdc077dedcd97012eb6f11dd801c5b263569d |

## Exact-head re-review

The independent reviewer rechecked corrective code head `0b25ea6579710d231237fb9b95e0edd426543be8`, including the final runbook and FR-055 edits. All ten findings remain fixed. The scoped Quoin digest, hash and pin scan found no new normative tracking-record obligation. **PASS** for this corrective scope.
