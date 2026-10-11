---
id: SR-251
title: "SAPHO-33 synthetic acceptance gap analysis"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho PR 55 at 94354115fbcb4a52c34e390c487b01548a8f572e; FR-069 through FR-071 and IT-012"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/IT-012"
    type: references
---
# SR-251: SAPHO-33 synthetic acceptance gap analysis

## Summary

`quire matrix --scope . --format json` exited 0 and found all 18 FR acceptance criteria tagged. Inspected the actual bound tests and implementation. The evidence covers basic receipt identity, a zero-margin tie, six-bar GO/NO-GO, primary-inclusive disagreement, output-set refusal, and 0/1/2 CLI exit paths. The following tags overstate subcase coverage.

## Verdict

**CONDITIONAL; HOLD for acceptance.** Complete the missing fixtures and resolve replay provenance before seeking maintainer review.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | FR-071-AC-5 and IT-012-SC-07 have no custom-backend live versus exact replay gate fixture. The tagged test uses only an offline graph and checks refusal of duplicate empty recordings. Equal deterministic recordings are rejected in production. | `crates/sapho-cli/tests/invocation.rs:1491-1636`; `crates/sapho-cli/src/command.rs:466-483` |
| FND-002 | medium | FR-069-AC-3 is tagged, but the tests change a label and pass a mismatched graph identity directly; they do not change an input, selected split, or graph literal through CLI registration/evaluation while asserting zero case dispatch. | `crates/sapho-evidence/src/gate.rs:655-725`; `crates/sapho-cli/tests/invocation.rs:1249-1485` |
| FND-003 | medium | FR-069-AC-4 and FR-071-AC-3 are tagged, but CLI tests cover an existing receipt/report and edited bar only. They omit malformed/unknown gate fields, missing/invalid bars, write failure, development-split refusal and graph/Dataset identity drift at the CLI boundary with a zero-call assertion. | `crates/sapho-cli/tests/invocation.rs:1249-1485`; FR-069-AC-4; FR-071-AC-3; IT-012-SC-06 |
| FND-004 | medium | The two-output IT-012 fixture specifies two Probability outputs with the first passing and second below a bar. The implemented CLI fixture uses a single case, Probability plus Boolean outputs, and only checks both names plus whole-graph exit 1; it does not prove the stated bar behavior. | `crates/sapho-cli/tests/invocation.rs:1494-1590`; FR-070-AC-7; IT-012-SC-05 |
| FND-005 | low | IT-012 specifies four-case tie and eight-case GO arithmetic, while the test fixtures use a two-case CLI graph and a four-case pure calculator. They establish some formulas, but do not check the specified eight-case ECE `1/4`, margin `3/8`, and false recall `3/4` together. | `spec/modules/evidence/integration/IT-012.md:22-30`; `crates/sapho-evidence/src/gate.rs:725-850`; `crates/sapho-cli/tests/invocation.rs:1254-1290` |

## Coverage

- Plan completion: not assessed.
- Quoin matrix: all 18 FR criteria tagged; no missing tag in this scope. Tags alone did not establish the missing subcases above.
- `sapho-evidence` still has only `sapho-core` as a workspace-crate dependency; Dataset serde/validate remains unchanged in SAPHO-33 commits.
- Quoin #687 parity merged SHA and focused receipt remain external dependencies; no live EARS 1,000-case promotion is claimed.
- No tracker or PR comment was posted under the review dispatch's read-only instruction.
