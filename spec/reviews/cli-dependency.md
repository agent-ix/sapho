---
id: SR-018
title: "CLI and selectors dependency review"
type: SpecReview
analysis: dependency
scope: "sapho@7d04780960a0404f64b9c7560fc4a998cac60842; full authorized CLI/format extension spec diff: spec/spec.md, FR-007/TC-007/IT-001, FR-030..FR-042 and owning TC/US/StR/module indexes, NFR-005, docs/yaml-authoring-assessment.md; existing owning core/graph/runtime/logic/recording/Jev contracts read for consistency"
review_set: subset
---
# SR-018: CLI and selectors dependency review

## Summary

Checked explicit prerequisite edges and separation of enabling contracts from user capabilities. The owning module graph is acyclic and permits implementing shared boundaries before consumer workflows.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No findings (placeholder). | FR-007; FR-030..FR-042; NFR-005 |

## Analysis

Enablement: FR-007 (loader), FR-032 (plain typed decoder), FR-030/031 (data assembly) and existing core/runtime/backend/recording contracts. Features: FR-033/034 invocation, FR-035 record/replay, FR-036 labelled measurement, FR-037 candidate comparison, FR-038 training export, FR-039..041 acquisition and FR-042 skills. Ordering: typed boundaries and loader -> compile/inspect/run -> record/replay and measurement -> tuning/export -> skills. Selection depends only on core decoding/identity and file-selection prerequisites for Git; it does not depend on engine execution. Evidence pure calculations depend only on core types, while invocation orchestration depends on the CLI runtime boundary. Consumer migration depends on the final loader contract/source availability; it cannot change core ownership. The Linear five-stage sequence is coordination order, not a claim that every selector requires EARS implementation. No cycles were found in the explicit depends_on graph.

## Verdict

No dependency cycles or missing prerequisite contracts found in the scoped requirements. This is a pre-implementation review: TC scenarios are planned verification, not claims of passing Rust tests. Code/test trace coverage is checked at implementation handoff.
