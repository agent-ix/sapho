---
id: SR-017
title: "CLI and selectors scope-boundary review"
type: SpecReview
analysis: scope-boundary
scope: "sapho@7d04780960a0404f64b9c7560fc4a998cac60842; full authorized CLI/format extension spec diff: spec/spec.md, FR-007/TC-007/IT-001, FR-030..FR-042 and owning TC/US/StR/module indexes, NFR-005, docs/yaml-authoring-assessment.md; existing owning core/graph/runtime/logic/recording/Jev contracts read for consistency"
review_set: subset
---
# SR-017: CLI and selectors scope-boundary review

## Summary

Allocated responsibilities across the unchanged engine crates and the new host/evidence/acquisition surfaces. The evidence specification needs to state which parts are pure calculations and which run in the CLI host.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | FR-036..038 mention paths, graph execution and exports while their owner sapho-evidence is declared pure/core-only. Without an allocation statement, filesystem/runtime dependencies could enter the evidence crate. | evidence/spec.md; FR-036..038 |

## Analysis

Allocation: graph loading owns FR-007; runtime owns FR-030/031; core owns FR-032; CLI host owns FR-033..035 and orchestration for FR-036..038; pure evidence owns dataset validation, scoring, ranking and export-record construction; selection owns FR-039..041 as infrastructure outside async runtime; plugin skills own FR-042 as caller workflow. Dependencies: YAML/JSON parser behavior is guaranteed by conformance tests; TypeSafe SDK/provider response behavior remains under the existing adapter contract and is not a model correctness guarantee; Git argument/output/deadline handling is guaranteed by process seam and temporary-repository tests while Git implementation is external; label truth is assumed from caller provenance; filesystem access is explicitly rooted and bounded. Domain extractors, EARS migration and model training execution remain external. No copied consumer data belongs in Sapho.

## Verdict

Revise the identified contracts before implementation. This is a pre-implementation review: TC scenarios are planned verification, not claims of passing Rust tests. Code/test trace coverage is checked at implementation handoff.
