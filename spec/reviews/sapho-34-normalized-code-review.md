---
id: SR-263
title: "SAPHO-34 normalized code final review"
type: SpecReview
analysis: base
scope: "agent-ix/sapho SAPHO-34 code f01013cbdbd558c237812fcbfd819d7f346d43ec against main d1fe552068cb80073f471df1b43a5b4889ace117"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-067"
    type: references
  - target: "ix://agent-ix/sapho/FR-068"
    type: references
  - target: "ix://agent-ix/sapho/IT-011"
    type: references
---
# SR-263: SAPHO-34 normalized code final review

## Summary

Read-only code and acceptance review at clean head `f01013cbdbd558c237812fcbfd819d7f346d43ec`. Its two commits are based directly on merged SAPHO-34 spec at `d1fe552`; the temporary SAPHO-21 cherry-pick is absent. Slice and drift calculations use the merged shared `measure` ECE/risk implementation, preserve Dataset types, validate recording exports, retain repeat counts and refuse exact typed Question mismatches. One host preflight gap remains.

## Verdict

**HOLD** for IT-011-SC-07 until FND-001 is fixed and the affected focused/full gates rerun. The supplied full system-access CI and focused tests passed at this head, but do not assert the required error precedence. No build was run in this review.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | `compare_recordings` validates selector/window lengths, then decodes and projects both recordings before rejecting a nonfinite/out-of-range threshold or identical window names (`crates/sapho-cli/src/drift.rs:85-100`). `compare_confidence_windows` checks those controls only after projection (`crates/sapho-evidence/src/drift.rs:140-153`). Invalid controls paired with malformed recording bytes therefore return a recording error and perform avoidable projection. IT-011-SC-07 requires invalid input refusal before projection. Validate threshold and distinct names in the host before either recording is loaded, and assert the typed control error with malformed bytes. | FR-068-AC-4; IT-011-SC-07 |

## Coverage

- `crates/sapho-evidence/src/slice.rs`: typed missing/value keys, deterministic case-ID accumulation, explicit window membership, absent deltas, and per-slice metrics through shared `measure`. Existing synthetic tests check slice `1.0/0.5/0.75`, signed deltas, minimum flags, ECE/risk reuse and byte-identical ordering.
- `crates/sapho-cli/src/drift.rs:24-71`: `Recording::from_json` validates bounded exports; exact Question equality and answer kind are checked before confidence projection. Every retained matching exchange is visited, including repeats. `crates/sapho-evidence/src/drift.rs` uses sorted finite confidences, inclusive thresholds, consistent denominators and absent empty-window statistics.
- `/private/tmp/sapho34-normalized-evidence.log`: 21 tests pass; `/private/tmp/sapho34-normalized-cli-drift.log`: four tests pass; `/private/tmp/sapho34-normalized-ci-system.log`: final `head=f01013cbdbd558c237812fcbfd819d7f346d43ec exit=0`. Existing invalid-threshold CLI fixture uses valid recordings and asserts only `.is_err()`, so it cannot detect FND-001.
- `git diff --check origin/main..HEAD` exits 0 and the worktree is clean. No build, source edit or push performed.
