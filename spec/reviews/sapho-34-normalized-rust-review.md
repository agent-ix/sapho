---
id: SR-261
title: "SAPHO-34 normalized Rust review"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho SAPHO-34 code f01013cbdbd558c237812fcbfd819d7f346d43ec versus main d1fe552068cb80073f471df1b43a5b4889ace117"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-067"
    type: references
  - target: "ix://agent-ix/sapho/FR-068"
    type: references
  - target: "ix://agent-ix/sapho/IT-011"
    type: references
---
# SR-261: SAPHO-34 normalized Rust review

## Summary

Read-only review of the clean eight-file normalized diff. I checked typed serialization, arithmetic and integer bounds, deterministic ordering, retained-exchange accounting, host projection privacy, resource handling and shared ECE behavior. No build was run; the parent reported full CI passing at this head.

## Verdict

**HOLD** on one host preflight gap. The core slice and drift calculations otherwise follow the reviewed contracts.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | `compare_recordings` checks selector and window identity lengths, then fully decodes and projects both recordings before checking whether the threshold is finite/in range or the two window names are distinct. Those checks occur only inside `compare_confidence_windows` at `crates/sapho-evidence/src/drift.rs:146-150`. Consequently an invalid threshold or identical names can trigger bounded but avoidable recording work, and malformed recording bytes win the error precedence. IT-011-SC-07 explicitly requires invalid input to refuse before projection. Validate both cheap controls at `crates/sapho-cli/src/drift.rs:85-89` before calling `project_recording_confidences`; add a test using invalid controls plus malformed bytes that asserts the typed input refusal. | FR-068-AC-4; IT-011-SC-07 |

## Coverage

- `crates/sapho-evidence/src/slice.rs` sorts each selected subset by `ItemId` before calling the existing `measure`, uses `BTreeMap`/`BTreeSet` for named partitions, keeps `SliceKey::Missing` distinct and ordered before literal values, and obtains ECE/risk rows from shared `Metrics::Probability`. It checks window assignments for unknown, duplicate and missing case IDs. Validated Dataset cardinality stays within `u32`, so report counts and conversions are bounded.
- `crates/sapho-evidence/src/drift.rs` validates report identities and finite unit-range confidences, checks that projected counts sum to retained exchanges, sorts confidence values before summing, and leaves empty-window statistics and deltas absent. An inclusive threshold is used. Bounded `u32` matching counts make its count conversion safe.
- `crates/sapho-cli/src/drift.rs` loads each recording through `Recording::from_json(bytes, max_bytes)` and projects only typed confidence values and bounded selector identities. It does not copy request/response bodies, endpoint configuration or credentials into the report. Repeated retained exchanges contribute separately. Question definition and answer kind are checked before pooling.
- `crates/sapho-evidence/Cargo.toml` retains `sapho-core` as its sole workspace-crate dependency. No new unsafe, lock, async/blocking bridge, or production caller-data `unwrap` was introduced. `git diff --check d1fe552..HEAD` exits 0; the worktree is clean. The existing tests cover slice ordering, window membership, shared ECE, synthetic confidence drift, repeated exchanges and question mismatch, but the invalid-control host test asserts only an unspecified error after valid recording projection.
