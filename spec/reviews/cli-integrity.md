---
id: SR-016
title: "CLI and evidence integrity review"
type: SpecReview
analysis: integrity
scope: "sapho@7d04780960a0404f64b9c7560fc4a998cac60842; full authorized CLI/format extension spec diff: spec/spec.md, FR-007/TC-007/IT-001, FR-030..FR-042 and owning TC/US/StR/module indexes, NFR-005, docs/yaml-authoring-assessment.md; existing owning core/graph/runtime/logic/recording/Jev contracts read for consistency"
review_set: subset
---
# SR-016: CLI and evidence integrity review

## Summary

Checked completeness, consistency, atomicity and testability. Measurement needs a per-output denominator and a explicit incomplete-run invocation outcome; label provenance is declared evidence, not independently verified truth.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | FR-036 describes coverage without fixing per-output aggregation and the CLI outcome when some labelled outputs fail. An incomplete run could appear to be a successful accuracy result. | FR-036; FR-034 |

## Analysis

Traceability: FR-030/031 -> US-003/StR-003; FR-032 -> US-001/StR-001; FR-033..035 -> US-007/StR-007; FR-036..038 -> US-008/StR-008; FR-039..041 -> US-009/StR-009; FR-042 -> US-010/StR-010. All have Test criterion methods and planned TCs. NFR-005 is referenced by each new FR. External Git has a declared minimum/detection/refusal; model concurrency/retry and credential ownership are explicit. Single-schema YAML/JSON avoids conflicting encodings. Measurements must retain declared label provenance and state that Sapho cannot verify the truth or independence of a user-supplied label. Acquisition and persistence failures are refusals, with no context cropping or silent overwrite.

## Verdict

Revise the identified contracts before implementation. This is a pre-implementation review: TC scenarios are planned verification, not claims of passing Rust tests. Code/test trace coverage is checked at implementation handoff.

## Dispositions

FND-001: fixed 20f522b9246b3281eec964285f3a7b17380dc5b9; owning contract updated before implementation.

## Final Verdict

Pass for implementation after the documented fixes. All four selected review lenses have validated artifacts; runtime/test evidence remains a later implementation obligation.
