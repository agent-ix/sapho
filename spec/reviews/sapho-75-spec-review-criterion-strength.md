---
id: SR-039
title: "Spec criterion-strength analysis (SAPHO-75)"
type: SpecReview
analysis: criterion-strength
scope: "agent-ix/sapho@83980c847c8ed97cef57e56d33fa6b09375d151d; spec/spec.md, spec/modules/core (FR-048, NFR-001, TC-048), spec/modules/evidence (FR-036, FR-038, TC-036, US-008), spec/modules/ollama/**; ticket SAPHO-75"
review_set: subset
---
# SR-039: Spec criterion-strength analysis

## Summary

Ticket: SAPHO-75. PR: agent-ix/sapho#19 at 83980c847c8ed97cef57e56d33fa6b09375d151d. Method: spec-review/criterion-strength (reviewer run 764c04b7-2ad7-4bef-b559-3f11ebcafc0c, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

Whether each acceptance criterion can fail. Jev was not used: this pass is a manual reading of every AC row, so weakness judgments are reviewer judgment, not calibrated scores.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | FR-049-AC-2 'contains no byte sequence from the first input' cannot pass literally | spec/modules/ollama/functional/FR-049.md:47 |

## Analysis

### FND-001 (low, confidence high, untestable-ac)

Unit: FR-049-AC-2 at spec/modules/ollama/functional/FR-049.md:47. Related: FR-049.

> After two consecutive extractions with different inputs, the second captured body contains no byte sequence from the first input or first response and has no `context`, `messages`, `images`, `template`, `raw` or `suffix` member.

Any two JSON bodies share byte sequences (braces, member names, common words), so the literal criterion fails on every implementation, and a test writer will silently weaken it. Use planted sentinel strings in the first input and first response, as sapho-dataset FR-008-AC-2 does.

## Scope

| Unit | Path | Role |
|------|------|------|
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-1 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-2 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-3 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-4 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-5 | spec/modules/core/functional/FR-048.md | examined |
| FR-048-AC-6 | spec/modules/core/functional/FR-048.md | examined |
| NFR-001 | spec/modules/core/non-functional/NFR-001.md | examined |
| spec/modules/core/spec.md | spec/modules/core/spec.md | examined |
| FR-036 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-036-AC-1 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-036-AC-2 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-036-AC-3 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-036-AC-4 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-036-AC-5 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-038 | spec/modules/evidence/functional/FR-038.md | examined |
| FR-038-AC-1 | spec/modules/evidence/functional/FR-038.md | examined |
| FR-038-AC-2 | spec/modules/evidence/functional/FR-038.md | examined |
| FR-038-AC-3 | spec/modules/evidence/functional/FR-038.md | examined |
| US-008 | spec/modules/evidence/usecase/US-008.md | examined |
| FR-049 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-1 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-2 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-3 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-4 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-049-AC-5 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-050 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-050-AC-1 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-050-AC-2 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-050-AC-3 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-050-AC-4 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-050-AC-5 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-051-AC-1 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-051-AC-2 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-051-AC-3 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-051-AC-4 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-052-AC-1 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-052-AC-2 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-052-AC-3 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-052-AC-4 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-053 | spec/modules/ollama/functional/FR-053.md | examined |
| FR-053-AC-1 | spec/modules/ollama/functional/FR-053.md | examined |
| FR-053-AC-2 | spec/modules/ollama/functional/FR-053.md | examined |
| FR-053-AC-3 | spec/modules/ollama/functional/FR-053.md | examined |
| FR-053-AC-4 | spec/modules/ollama/functional/FR-053.md | examined |
| IT-007 | spec/modules/ollama/integration/IT-007.md | examined |
| spec/modules/ollama/spec.md | spec/modules/ollama/spec.md | examined |
| StR-012 | spec/modules/ollama/stakeholder/StR-012.md | examined |
| StR-012-VC-1 | spec/modules/ollama/stakeholder/StR-012.md | examined |
| StR-012-VC-2 | spec/modules/ollama/stakeholder/StR-012.md | examined |
| US-012 | spec/modules/ollama/usecase/US-012.md | examined |
| spec/spec.md | spec/spec.md | examined |
| FR-005 | spec/modules/core/functional/FR-005-validate-model-answers.md | context_only |
| FR-006 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | context_only |
| FR-007 | spec/modules/graph/functional/FR-007-load-declarative-graph-configuration.md | context_only |
| FR-035 | spec/modules/cli/functional/FR-035.md | context_only |

## Verdict

0 high, 0 medium, 1 low; blocking: none.

