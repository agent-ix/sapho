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

The changed public Sapho contracts define an observational graph operation, decision-first execution and shared RunLimits, exact recording behavior, pure comparison against existing Dataset labels, and CLI reporting. The review checked the six SAPHO-27 ticket acceptance checks against FR-057 through FR-060 and IT-008, identifier uniqueness, links, unhappy paths, limit boundaries, split handling and the existing crate boundary. Planner review found three contract defects and a later wording ambiguity; the dispositions below are incorporated in this revision. This review makes no claim that the new behavior works yet.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | Resolved: shadow state/questions/guard inputs now reject direct and transitive dependencies on champion or any ask; an ask-free shared producer remains allowed and has a compile-refusal AC. | FR-057-AC-5; IT-008-SC-01 |
| FND-002 | medium | Resolved: a zero-margin tie is false; gain must be strictly positive and meet the declared margin, with equality cases covered. | FR-059-AC-3; IT-008-SC-05 |
| FND-003 | medium | Resolved as an explicit availability boundary: ECE reuses SAPHO-21's shared metric when present; before that, it is not-computed with a reason and cannot produce a win. | FR-059-AC-2; IT-008-SC-05 |
| FND-004 | low | Implementation evidence is pending: new and amended ACs are untagged in `quire matrix` before coding; bind each to an observable test before claiming completion. | FR-027-AC-7; FR-057 through FR-060 |
| FND-005 | low | Resolved: a shared ask-free producer remains a valid shadow input even if it is also mapped to a decision output; only transitive ask-derived values are refused. | FR-057-AC-5; IT-008-SC-01 |

## Review Evidence

The compiler contract rejects both shadow-to-decision dependencies and decision-ask-to-shadow dependencies, while allowing common ask-free inputs. The runtime contract preserves completed decision outputs and exit status after shadow failure, schedules decision work first, and charges shadow calls to the shared limits. Recording uses existing exact request identity and keeps failed calls in trace only. The evidence contract compares one label per case/output on matching case sets, reports agreement and Brier by label kind, and makes ECE availability explicit. It requires positive gain for a win, applies the declared margin, and marks development comparisons not promotable. The CLI contract carries the result through record, replay and measure. IT-008 names success, contamination refusal, failure, limits, split, tie and role-swap procedures with per-step success criteria.

For this revision, `quire validate --scope . "spec/**/*.md"` and `git diff --check` exited 0. `quire matrix --scope . --format tsv` showed 20 scoped criteria as `untagged`, pending implementation tests. No source tests were run for this specification change.
