---
id: SR-041
title: "Spec risk-complexity analysis (SAPHO-75)"
type: SpecReview
analysis: risk-complexity
scope: "agent-ix/sapho@83980c847c8ed97cef57e56d33fa6b09375d151d; spec/spec.md, spec/modules/core (FR-048, NFR-001, TC-048), spec/modules/evidence (FR-036, FR-038, TC-036, US-008), spec/modules/ollama/**; ticket SAPHO-75"
review_set: subset
---
# SR-041: Spec risk-complexity analysis

## Summary

Ticket: SAPHO-75. PR: agent-ix/sapho#19 at 83980c847c8ed97cef57e56d33fa6b09375d151d. Method: spec-review/risk-complexity (reviewer run 764c04b7-2ad7-4bef-b559-3f11ebcafc0c, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

Technical risk and volatility of the requirements before tasking.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | Core-side JSON Schema 2020-12 compile/validate vs no-transport rule: remote $ref resolution and the enforcing mechanism are unstated | spec/modules/core/functional/FR-048.md:58 |

## Analysis

### FND-001 (medium, confidence medium, other)

Unit: FR-048-AC-6 at spec/modules/core/functional/FR-048.md:58. Related: FR-048.

> The core crate's dependency list contains no async runtime, HTTP or other transport crate after the port is added.

FR-048 puts schema compilation and validation in sapho-core (refuse a schema that does not compile; check every returned value), and AC-6 forbids any HTTP or transport crate in core. Common Rust 2020-12 validators resolve remote `$ref` over HTTP and from files by default, which would either add a transport dependency or let a schema make core fetch a URL. The spec also does not say how core guarantees validation 'before any implementation is invoked' (a core-owned call wrapper versus a trait method an implementation can bypass). State that only in-document `$ref` is resolved (external refs refused with Config) and name the core entry point that wraps the trait.

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

0 high, 1 medium, 0 low; blocking: none.

## New findings (disposition pass 1)

Reviewed at agent-ix/sapho@2812aa1800068709943743661d947c4d1ad21692.

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-002 | low | Score questions with 11 or more levels always fail `ambiguous_answer_token` | spec/modules/ollama/functional/FR-054.md:48 |

### FND-002 (low, confidence medium, other)

Unit: FR-054 at spec/modules/ollama/functional/FR-054.md:48. Related: FR-054.

> If the generated token at an answer position is a prefix of more than one allowed value, then the adapter SHALL return `InvalidAnswer` with reason `ambiguous_answer_token` for that question; label sets whose values begin with distinct tokens avoid it.

Score questions with 11 or more levels always fail `ambiguous_answer_token`. Score values are decimal strings, so with level_count >= 11 the token `1` prefixes `1` and `10`; whenever the model starts with `1` the question fails. Refuse such a question block at binding time with a clear reason, or encode levels so first tokens differ.


## Dispositions

Round 1, reviewed at agent-ix/sapho@2812aa1800068709943743661d947c4d1ad21692.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 2812aa1: External $ref refused; single core call path `sapho_core::extract`; AC-4 and AC-6 extended. |

## Dispositions (round 2)

Round 2, reviewed at agent-ix/sapho@a34c54c8909d6f73a6a2258e4419fa2fa5b0af38.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-002 | fixed | a34c54c: First-byte distinctness refused up front; Score capped at 10 levels (`too_many_levels`); FR-054-AC-8. |
