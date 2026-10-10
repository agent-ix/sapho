---
id: SR-056
title: "Base review of SAPHO-27 shadow challenger specification"
type: SpecReview
analysis: base
scope: "SAPHO-27; spec/spec.md; FR-027-AC-7; FR-057..060; IT-008; affected module indexes"
review_set: base
---
# SR-056: Base review of SAPHO-27 shadow challenger specification

## Summary

The changed public Sapho contracts define an observational graph operation, decision-first execution and shared RunLimits, exact recording behavior, pure comparison against existing Dataset labels, and CLI reporting. The review checked the six SAPHO-27 ticket acceptance checks against FR-057 through FR-060 and IT-008, identifier uniqueness, links, unhappy paths, limit boundaries, split handling and the existing crate boundary. The design is ready for implementation review; it makes no claim that the new behavior works yet.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No unresolved specification defect found in the scoped base review. The 19 new or amended acceptance criteria are untagged in `quire matrix` because implementation and executable tests have not begun; bind each criterion to an observable test during coding before claiming completion. | FR-027-AC-7; FR-057 through FR-060 |

## Review Evidence

The compiler contract rejects decision dependence on shadows and invalid observation mappings. The runtime contract preserves completed decision outputs and exit status after shadow failure, schedules decision work first, and charges shadow calls to the shared limits. Recording uses existing exact request identity and keeps failed calls in trace only. The evidence contract compares one label per case/output on matching case sets, reports agreement, Brier and ECE by label kind, applies an explicit margin, and marks development comparisons not promotable. The CLI contract carries the same result through record, replay and measure. IT-008 names success, failure, limit, split and role-swap procedures with per-step success criteria.

`quire validate --scope . "spec/**/*.md"` exited 0. `quire matrix --scope . --format tsv` showed all 19 scoped criteria as `untagged`, as expected before implementation. No source tests were run for this specification change.
