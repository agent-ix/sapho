---
id: SR-063
title: "SAPHO-26 router specification review"
type: SpecReview
analysis: base
scope: "FR-077, FR-078 and IT-015 against current origin/main"
review_set: subset
---
# SR-063: SAPHO-26 base review

## Summary

Reviewed the current guarded Optional ports, Coalesce compiler/runtime contract, Ask skip traces, example graph locations, user guide and CI `docs-examples` gate on `origin/main` `e77b81eddd90b606561f2ed606cde74f133ea937`. This is pre-implementation specification evidence only.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Coalesce requires an unwrapped default, causing an unconditional backend call in an N-way guarded router. | FR-077 |
| FND-002 | medium | First-wins would hide overlapping guards and a zero-present merge could fabricate a fallback. | FR-077, IT-015 |
| FND-003 | medium | Merging Answers without actual-model Text could misreport which backend answered. | FR-077, FR-078 |
| FND-004 | medium | The user guide has no three-way router and existing CI does not explicitly validate two new graph files. | FR-078, IT-015 |
| FND-005 | low | Existing guarded fast/expert Coalesce escalation is already working and must remain distinct from routing. | FR-077, FR-078 |

## Dispositions

FND-001: FR-077 defines `merge_present` over 2–32 named Optional(T) inputs with no default. FND-002: exact-one presence is required; zero/multiple return structured InvalidValue, tested with gap and overlap fixtures. FND-003: router graph merges Answers and model Text separately under the same Ask guards and projects probability from merged Answers. FND-004: FR-078 names both example paths, user-guide links and CI validation commands; IT-015 checks them. FND-005: escalation example and regression fixture retain fast-always, expert-conditional Coalesce semantics.

## Verdict

Ready for planner review of the public operator, graph and documentation contract. `quire validate --scope . 'spec/**/*.md'` exited 0; `quire matrix --scope .` exited 0 with all ten new ACs untagged; `git diff --check` exited 0. `filament open` could not dispatch the IDE URL through the OS opener (error -10661). IT-015 and both example graphs are planned implementation evidence, not executed tests. Dataset serde/validate and EARS production wiring are outside this specification.
