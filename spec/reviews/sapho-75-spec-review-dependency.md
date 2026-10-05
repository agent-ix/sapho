---
id: SR-037
title: "Spec dependency analysis (SAPHO-75)"
type: SpecReview
analysis: dependency
scope: "agent-ix/sapho@83980c847c8ed97cef57e56d33fa6b09375d151d; spec/spec.md, spec/modules/core (FR-048, NFR-001, TC-048), spec/modules/evidence (FR-036, FR-038, TC-036, US-008), spec/modules/ollama/**; ticket SAPHO-75"
review_set: subset
---
# SR-037: Spec dependency analysis

## Summary

Ticket: SAPHO-75. PR: agent-ix/sapho#19 at 83980c847c8ed97cef57e56d33fa6b09375d151d. Method: spec-review/dependency (reviewer run 764c04b7-2ad7-4bef-b559-3f11ebcafc0c, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

Requirement and crate dependency edges, cross-repo ix:// references, enablement versus feature work.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No findings (placeholder) | - |

## Analysis

New edges: sapho-ollama depends only on sapho-core (NFR-001 scope and the master flowchart updated); FR-049..053 depend_on chain is acyclic (FR-048 <- FR-049 <- FR-050/051/052 <- FR-053). No Sapho artifact references sapho-dataset.
## Scope

| Unit | Path | Role |
|------|------|------|
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| NFR-001 | spec/modules/core/non-functional/NFR-001.md | examined |
| spec/modules/core/spec.md | spec/modules/core/spec.md | examined |
| FR-036 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-038 | spec/modules/evidence/functional/FR-038.md | examined |
| US-008 | spec/modules/evidence/usecase/US-008.md | examined |
| FR-049 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-050 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-053 | spec/modules/ollama/functional/FR-053.md | examined |
| IT-007 | spec/modules/ollama/integration/IT-007.md | examined |
| spec/modules/ollama/spec.md | spec/modules/ollama/spec.md | examined |
| StR-012 | spec/modules/ollama/stakeholder/StR-012.md | examined |
| US-012 | spec/modules/ollama/usecase/US-012.md | examined |
| spec/spec.md | spec/spec.md | examined |
| FR-005 | spec/modules/core/functional/FR-005-validate-model-answers.md | context_only |
| FR-006 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | context_only |
| FR-007 | spec/modules/graph/functional/FR-007-load-declarative-graph-configuration.md | context_only |
| FR-035 | spec/modules/cli/functional/FR-035.md | context_only |

## Verdict

No findings from this method.

