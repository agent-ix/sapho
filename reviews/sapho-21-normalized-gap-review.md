---
id: SR-257
title: "Gap analysis of normalized SAPHO-21 evidence code"
type: SpecReview
analysis: gap-analysis
scope: "SAPHO-21 normalized code at b7644903cf0bfc784361be6cfe4021fca328be5f; FR-065, FR-066, IT-010, shared roster/shadow ECE"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-065"
    type: references
  - target: "ix://agent-ix/sapho/FR-066"
    type: references
---
# SR-257: Gap analysis of normalized SAPHO-21 evidence code

## Summary

Read-only acceptance re-review at clean head `b7644903cf0bfc784361be6cfe4021fca328be5f`. The strict computed matrix tags all ten FR-065/066 criteria. Synthetic ten-case ECE, decile boundaries, six risk rows, invalid thresholds, empty/unsupported measurements, split isolation and Dataset round trip are covered by committed tests. The shared ECE path now reaches roster and shadow comparisons; one absent-reason branch still describes the pre-SAPHO-21 state after the metric is available. Plan completion: not assessed.

## Verdict

**HOLD** for the normalized Sapho code until FND-001 is resolved and the affected tests/gates rerun. The supplied full CI and focused evidence receipts passed at this head; this read-only review did not rerun builds. Quoin parity PR #687 remains open separately, so its merged SHA and focused passing parity output are still required for ticket completion.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | `roster()` sets `ece_absent_reason` to `not_computed` whenever shared ECE is `None` (`crates/sapho-evidence/src/roster.rs:666`). After SAPHO-21, a Boolean/unsupported output has no applicable ECE and a zero-scored Probability output has intentionally absent ECE; neither means the shared metric has not been computed or is unavailable. A roster with such an output therefore reports a false reason in its versioned evidence. Distinguish inapplicable type from zero scored cases, or omit the reason when the typed measurement already explains absence, and add both cases to the roster tests. | FR-062 ECE behavior; FR-065-AC-3/4; FR-064 roster absent-metric reasons |

## Coverage

`quire matrix --strict --scope . --format tsv` exited 0 and reports five tagged FR-065 criteria and five tagged FR-066 criteria. `sapho_21_ten_case_ece_parity` asserts ECE `0.33`, bucket 9 `3/2/0.95`, bucket 8 `1/1/0.85`, and all six risk counts. Boundary/override tests assert inclusive thresholds and invalid-list refusal. The original Dataset fields, serde and validation code remain unchanged; existing negative Dataset tests and the original `measure` signature remain in use. Shadow ECE is computed on the matched scored case set and its tested fixture shows a concrete improvement; roster ECE equals the same `measure` metric for a scored Probability entry. This review found no test for the roster ECE reason on a Boolean or zero-scored entry, which exposes FND-001.

Quoin PR #687 is OPEN at `89c37f9b3af0f04cd00acd07338477742f116c57`. Its parity test is present with a passing GitHub gate, but the merged parity receipt is external to this Sapho code finding.

Plan completion: not assessed.
