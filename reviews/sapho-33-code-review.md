---
id: SR-249
title: "SAPHO-33 promotion gate code review"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho PR 55 at 94354115fbcb4a52c34e390c487b01548a8f572e; SAPHO-33 code commits 1e6e67e and 9435411"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-071"
    type: references
---
# SR-249: SAPHO-33 promotion gate code review

## Summary

Reviewed only SAPHO-33 code commits after the cleared spec, excluding the SAPHO-21 and SAPHO-25 cherry-picks. The gate is registered with create-new semantics before case execution, verifies byte and semantic identities, evaluates all six bars, and assigns 0/1/2 according to complete GO, complete NO-GO, and incomplete/refused evidence. The exact-head full CI receipt is `/private/tmp/sapho-33-code-final-ci.log` (exit 0). No build was run in this review.

## Verdict

**CONDITIONAL; HOLD for acceptance.** Replay handling prevents a valid deterministic repeated-result scenario, and the tagged CLI test does not cover custom-backend live/replay parity.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | The CLI hashes normalized Recording content and refuses any repeated hash. Independently captured requests to a deterministic backend can produce identical typed recordings, so their exact replay is refused before case evaluation despite distinct source captures. Distinct content is neither necessary nor proof of independent capture. Establish a provenance rule that admits legitimate identical responses, and test captured custom-backend primary plus two additional recordings through live and replay. | `crates/sapho-cli/src/command.rs:466-483`; `crates/sapho-cli/tests/invocation.rs:1591-1636`; FR-071-AC-5; IT-012-SC-07 |
| FND-002 | medium | The only CLI test tagged FR-071-AC-5 uses an offline input graph and empty recordings and asserts rejection. It never invokes a custom backend, captures its responses, or compares live and exact replay gate results. The named acceptance check has no executable supporting fixture. | `crates/sapho-cli/tests/invocation.rs:1491-1636`; FR-071-AC-5; IT-012-SC-07 |

## Rust lane

The new public evidence types use typed `GateBars`, `PromotionGate`, `GateRegistration`, `GateReport`, and `BarStatus`; serde rejects unknown gate fields and duplicate output names. Nonfinite/out-of-range mandatory bars refuse. CLI reads bounded artifacts, claims exclusive destinations, and retains complete case evidence. Inspected `crates/sapho-evidence/src/gate.rs:16-224` for schema and hashing, `:335-515` for comparisons and verdict, and `crates/sapho-cli/src/command.rs:397-559` for dispatch and exit policy. The replay provenance finding is also a Rust-lane finding: typed content equality cannot establish whether equal responses came from distinct calls.

## Coverage

- Quoin matrix reports all 18 FR-069/070/071 criteria tagged, but several tags point to tests that do not exercise the named subcase; separate acceptance receipt is SR-251.
- No applicable AssuranceProfile was found in this branch's `spec/` tree.
- No tracker or PR comment was posted under the review dispatch's read-only instruction.

## Dispositions

Read-only source disposition at local, unpushed `cd05d0603deb3b65e2fc533b21d01ed5da2049d4`. The fix round has not passed focused or full build gates; these are provisional source judgments.

| FND | Outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | `cd05d06`: `command.rs:471-500` now rejects reuse of the same canonical path or Unix device/inode while allowing equal typed contents in separate files. `invocation.rs:1614-1665` exercises equal-content copies and hard-link refusal. |
| FND-002 | fixed | `cd05d06`: `invocation.rs:1669-1826` runs three independent custom `ModelBackend` captures, checks 12 backend calls, confirms equal recording bytes, then compares the pure live gate report with CLI exact replay output. |

## New findings (disposition pass 1)

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-003 | low | The added custom-backend test compares direct live `Runner`/`evaluate_gate` evidence to CLI replay; it does not invoke CLI's live `gate-evaluate` path with the custom backend. The host-injected seam makes that exact process fixture unavailable without a CLI injection mechanism. Source inspection shows both paths use the same `run_cases`/`measure`/`evaluate_gate` chain, so this is an evidence limitation rather than a demonstrated behavior defect. | `crates/sapho-cli/tests/invocation.rs:1669-1826`; `crates/sapho-cli/src/command.rs:488-554`; FR-071-AC-5 |
