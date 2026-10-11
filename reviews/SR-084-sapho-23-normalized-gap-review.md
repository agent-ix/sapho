---
id: SR-084
title: "SAPHO-23 normalized code acceptance review"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho@93012906343451fcddee053a0ea047070413ec9c; FR-079, FR-080, IT-016"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-079"
    type: references
  - target: "ix://agent-ix/sapho/FR-080"
    type: references
---
# SR-084: SAPHO-23 normalized code acceptance review

## Summary

All ten FR-079/080 criteria have computed Test Matrix binders. The implementation separately plans and validates the whole bounded grid before exclusive directory creation, publishes the manifest last, and leaves Dataset/recording interfaces unchanged. The unit and command tests cover three- and six-candidate order, literal metadata, deterministic manifest bytes, collisions and rollback, ordinary tune, and replay success/miss. Two explicit IT-016 checks remain unproven by the tests.

## Verdict

**CONDITIONAL (HOLD for acceptance).** The code may satisfy the requirements, but the current fixture does not establish the required two-command 20-candidate observation. Plan completion: not assessed.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | IT-016-SC-05 requires over-16 generated candidates to be passed through ordinary `tune` only with its own explicit `--max-candidates`. The test checks `plan(..., 20)` and tunes only six candidates; no test exercises the two-command 20-candidate boundary. Add a 5x4 public sweep/tune fixture proving default refusal and explicit 20 acceptance in both commands. | crates/sapho-cli/src/sweep.rs:574-586, crates/sapho-cli/tests/sweep.rs:211-316, FR-080-AC-3, IT-016-SC-05 |

## Coverage

Plan completion: not assessed

- `quire matrix --scope . --format json`: all ten FR-079/080 criteria tagged; exit 0, ambient catalog warnings only.
- `git diff --check 8b118321...HEAD`: exit 0. Worktree clean at the reviewed head.
- Three single-axis candidate trees are compared to base with only cutoff changed in `product_order_metadata_and_manifest_bytes_are_deterministic`; six two-axis files, coordinates, digests, item IDs/order and outer sources are asserted there and in `stock_sweep_validates_full_grid_and_never_overwrites_sources_or_destination`.
- `publisher_refuses_collisions_and_cleans_only_created_files` verifies file/directory collision and rollback after the first candidate write; manifest is last in `publish_with_hook`.
- The remaining invalid grid variants are covered in the private planner; the public CLI proves a late invalid Probability leaves its destination absent. `sweep` unconditionally completes planning before calling the publisher, so this is sufficient structural evidence for the other validation variants.
- `generated_candidates_tune_like_handwritten_and_replay_unchanged_requests` verifies ordinary six-candidate tune metric/ranking equality and common replay requests; `request_affecting_probability_literal_has_exact_replay_miss` verifies typed miss.
- No build or test execution was performed by this read-only reviewer.
