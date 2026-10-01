---
id: SR-020
title: "Data CLI implementation traceability"
type: SpecReview
analysis: gap-analysis
scope: "sapho@762233e51d0853795c511834d036f566490687c4; SAPHO-3/SAPHO-5 delivery scope, FR-007 and FR-030..042, owning TC files, NFR-005, spec/tests.md TM-001, changed source and tagged tests"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/TM-001"
    type: references
---
# SR-020: Data CLI implementation traceability

## Summary

Audited the authorized loader/data-workflow scope directly, as with the repository's previous confined contract review. The ordered Linear tickets are the delivery plan; no duplicate Plan/Task bundle was authored. Functional criteria are backed, but the new matrix needs explicit test-case tags before it can claim reconciled coverage.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | high | All 14 extension matrix test cases have real tests carrying their functional AC tags, but no explicit TC tag binds the matrix rows. Quire reports 0/14 matrix cases backed and 42 unbacked functional-coverage reference rows. Bind the existing tests to their actual owning TC IDs. | spec/tests.md:14 |

## Coverage

Quire 0.33.0 / engine 0.47.1 reconciled 129/129 functional acceptance criteria, including all 42 changed/new criteria (FR-007 and FR-030..042). Whole-corpus totals are 129/163: the additional 20 stakeholder demonstration criteria and 14 new matrix cases are not runtime functional criteria. No untracked source symbols or false complete statuses were reported. The matrix deliberately remains pending until full gates pass.

The installed CLI has no `matrix` subcommand; its real `coverage` engine is used without claiming an unavailable strict-matrix check. No optional independent semantic review was requested or run. Owning requirements cover the added core conversion, parser/assembly operations, host invocation, evidence measurement/tuning/export, selectors and skills; no source stub or behavior lacking an owning requirement was found.

Legacy NFR files use measurement tables rather than AC tables; ambient module diagnostics also identify intentionally absent inspection/interface/suite document classes. These are disclosed rather than represented as runtime-tested criteria. The scoped matrix does not claim entire stakeholder acceptance, live model quality or EARS consumer migration. SAPHO-4's consumer-owned migration must be verified after the source handoff.

## Verdict

FAIL until the matrix binding finding is fixed and the required two full gates pass. User policy prioritizes functional code and does not require creating an unused Plan bundle for this delivery; plan artifact completion is not claimed.
