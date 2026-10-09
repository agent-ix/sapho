---
id: SR-066
title: "Gap analysis of corrective Ollama identity evidence"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho@b155ab198cd836c86b170319ca8eb2311e6bb5a7; FR-052, FR-053, IT-007, crates/sapho-ollama/tests"
review_set: subset
---
# SR-066: Corrective Ollama identity gap analysis

## Summary

Ticket: SAPHO-141. The trace tags exist, but two tagged tests now assert an absent digest while their unchanged criterion or scenario still requires a present digest.

## Verdict

FAIL due to requirement and evidence disagreement. Plan completion: not assessed.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | FR-053-AC-1 tagged embedding test asserts no digest although the criterion requires one | crates/sapho-ollama/tests/embedding.rs:30 |
| FND-002 | high | IT-007-SC-01 tagged extraction test asserts no digest although the scenario requires one | crates/sapho-ollama/tests/extraction.rs:531 |

## Coverage

- Examined FR-052 AC-1 through AC-7, FR-053 AC-1, IT-007-SC-01 and the tagged tests. The changed FR-052 tags align with response-name behavior.
- `quoin matrix --repo . --json` was invoked for the computed matrix; semantic inspection found the two false-positive bindings above. Plan completion: not assessed.

## Dispositions

| FND | outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |
| FND-002 | fixed | 8499f3b4b40afcc47d6021aef13e227ea6b561be |

## Exact-head re-review

The independent reviewer rechecked corrective code head `0b25ea6579710d231237fb9b95e0edd426543be8` after the linked spec corrections and the final runbook and FR-055 edits. Both findings remain fixed. **PASS** for this corrective scope.
