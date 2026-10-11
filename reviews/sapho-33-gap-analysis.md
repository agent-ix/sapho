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

## Dispositions

Read-only source disposition at local, unpushed `cd05d0603deb3b65e2fc533b21d01ed5da2049d4`. Focused/full tests were not run in this pass under the machine-memory pause.

| FND | Outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | `cd05d06`: `invocation.rs:1669-1826` captures three separate custom backend runs and compares live gate evidence with CLI replay; equal content is now accepted by `command.rs:471-500`. |
| FND-002 | fixed | `cd05d06`: `invocation.rs:2007-2132` changes label, input, selected split, graph literal and gate whitespace at the CLI boundary. `command.rs:442-461` verifies identities before runners and cases exist. |
| FND-003 | fixed | `cd05d06`: `invocation.rs:2133-2196` covers malformed, unknown, absent and invalid bars, development gate and write failure; `:2079-2132` covers evaluation identity drift, while earlier fixture covers existing receipt/report destinations. |
| FND-004 | fixed | `cd05d06`: `invocation.rs:1828-2004` uses two Probability outputs over eight cases, asserts first output's six bars pass, second margin/recall fail, and whole graph exits 1. |
| FND-005 | fixed | `cd05d06`: `gate.rs:655-686` checks four-case tie and true recall zero; `invocation.rs:1828-1958` checks eight-case ECE 0.25, margin 0.375 and false recall 0.75 together. |

## New findings (disposition pass 1)

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-006 | low | The early-refusal CLI test infers zero case evaluation from absence of an output artifact; it does not use a backend call counter. The implementation verifies registration before constructing runners or entering `run_cases`, so the ordering is supported by source inspection. | `crates/sapho-cli/tests/invocation.rs:2079-2101`; `crates/sapho-cli/src/command.rs:442-515`; FR-071-AC-3 |
