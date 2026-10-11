---
id: SR-256
title: "SAPHO-25 normalized code review after shadow merge"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho PR 44; main f46fc39e0cef45ba5368fa32c498e305db79ce83..local cf86368dae0325d6d227815bfb35bee5c32d0095"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-061"
    type: references
  - target: "ix://agent-ix/sapho/FR-062"
    type: references
---
# SR-256: SAPHO-25 normalized code review after shadow merge

## Summary

Read-only code and Rust review of clean normalized local head `cf86368dae0325d6d227815bfb35bee5c32d0095` against main `f46fc39e0cef45ba5368fa32c498e305db79ce83`, after SAPHO-27 spec/code merged. The prior SAPHO-25 implementation and its tests remain present; range-diff shows the substantive code commits preserved with import and context adaptation. The merged shadow runtime creates a new ModelBackend inference path that the new roster observation sidecar does not capture.

## Verdict

**HOLD.** The combined implementation violates FR-061's one-observation-per-attempted-backend-call rule on a graph with a shadow ask. The prior exact-head `make ci` and independent PASS receipts at `0b902b9` did not exercise this post-merge composition. No full gate at normalized head is claimed.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | `run_observed` executes decision and shadow phases, but the shadow path calls `ModelBackend::infer` after `state.add_request()` without inserting its trace index into `state.started` or recording duration. `observation::collect` then filters by that started-index set and accepts only `Operation::Ask`, excluding `ShadowAsk` a second time. A successful or failed dispatched shadow call therefore has no `ModelCallObservation`, so roster call counts and per-binding actual-model/usage/latency evidence omit real attempted calls. Add a combined champion+shadow observed-run fixture asserting two call observations, actual identities, live/replay duration behavior and unchanged decision-output lineage. | `crates/sapho-runtime/src/engine.rs:181-223,544-570`; `crates/sapho-runtime/src/observation.rs:80-115`; FR-061 description/behavior; FR-062-AC-1; SAPHO-27 FR-058 |

## Coverage

- Prior SAPHO-25 SR-072/SR-073 code/gap PASS at old `0b902b9` reviewed roster semantics before SAPHO-27 was in its base; `git range-diff` of seven commits shows adaptations but no combined shadow-observation fixture.
- `contributing_responses` in CLI walks `Operation::Ask` for decision output lineage, which correctly excludes shadow observations from ordinary output attribution; the finding concerns attempted-call accounting in the separate sidecar.
- `git diff --check f46fc39..cf86368` passed; local branch was clean. GitHub #44 still pointed at old `0b902b9` on the spec branch and was `CONFLICTING` when checked; no new-head CI or maintainer review receipt exists.
- No build, edit, push, merge or tracker action was performed.
