---
id: SR-038
title: "Spec failure-domain analysis (SAPHO-75)"
type: SpecReview
analysis: failure-domain
scope: "agent-ix/sapho@83980c847c8ed97cef57e56d33fa6b09375d151d; spec/spec.md, spec/modules/core (FR-048, NFR-001, TC-048), spec/modules/evidence (FR-036, FR-038, TC-036, US-008), spec/modules/ollama/**; ticket SAPHO-75"
review_set: subset
---
# SR-038: Spec failure-domain analysis

## Summary

Ticket: SAPHO-75. PR: agent-ix/sapho#19 at 83980c847c8ed97cef57e56d33fa6b09375d151d. Method: spec-review/failure-domain (reviewer run 764c04b7-2ad7-4bef-b559-3f11ebcafc0c, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

Unstated failure modes: resume and idempotency, partial JSONL lines, concurrent writers, digest mismatch, truncation, identity confusion and correlated hashing.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | FR-050 assumes prompt_eval_count counts the whole prompt; with Ollama's prompt cache it counts only newly evaluated tokens | spec/modules/ollama/functional/FR-050.md:37 |
| FND-002 | low | Model digest resolved once per binding; a re-pull mid-run records a stale digest | spec/modules/ollama/functional/FR-052.md:34 |

## Analysis

### FND-001 (high, confidence medium, soundness)

Unit: FR-050 at spec/modules/ollama/functional/FR-050.md:37. Related: FR-052, sapho-dataset/FR-010, sapho-dataset/FR-023.

> If a response reports `prompt_eval_count + num_predict` above `num_ctx`, then the adapter SHALL return `TooLarge` with the retained raw exchange and usage, because the server can shorten a prompt to fit its context.

Every item of a task sends the same `system` instructions, so Ollama reuses the cached prefix and reports in prompt_eval_count only the tokens it evaluated (the item part); on a fully cached identical request (resume of the item in flight, a retried failed item) the field can be tiny or omitted. Consequences: the post-call TooLarge check (this line) under-counts and can miss a truncated prompt; estimate_exceeded (next bullet) almost never fires, so the ratio cannot be corrected from evidence; usage.input_tokens and status tokens-per-item are wrong; and the 'lacks prompt_eval_count -> InvalidAnswer' rule can fail a resumed item on every retry. Confidence medium: verify on Draco before coding SAPHO-32 (send two requests sharing a long system prompt and compare prompt_eval_count). Options: disable prefix reuse per request if Ollama allows it, treat prompt_eval_count as a lower bound and base the post-check on Ollama's truncation signal instead, or count tokens with the model tokenizer endpoint where available.

### FND-002 (low, confidence medium, other)

Unit: FR-052 at spec/modules/ollama/functional/FR-052.md:34. Related: FR-053, sapho-dataset/FR-014.

> When a binding sends its first generate request, the adapter SHALL first read the server's model list and resolve the binding's model name to its digest.

The digest is read from /api/tags only at the binding's first generate request. If the operator re-pulls or re-creates the tag during an overnight run, later labels carry the old digest while a different model answered. sapho-dataset FR-014 uses name+digest to decide same-source. Re-resolve when Ollama reports a load (load_duration > 0) or at each run start, and refuse with ModelMismatch on change.

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

1 high, 0 medium, 1 low; blocking: none.

