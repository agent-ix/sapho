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

## New findings (disposition pass 1)

Reviewed at agent-ix/sapho@2812aa1800068709943743661d947c4d1ad21692.

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-003 | medium | Silent truncation still goes undetected when the byte ratio over-estimates bytes per token for an input | spec/modules/ollama/functional/FR-050.md:36 |

### FND-003 (medium, confidence medium, soundness)

Unit: FR-050 at spec/modules/ollama/functional/FR-050.md:36. Related: FR-054, FR-049.

> If `estimated_input_tokens + num_predict` exceeds `num_ctx`, then the adapter SHALL return `TooLarge` without sending a request.

Silent truncation still goes undetected when the byte ratio over-estimates bytes per token for an input. FR-050 now relies on the pre-send estimate because the server truncates silently. The ratio 3.0 is conservative for English (4.3 bytes/token measured) but not for every input: dense code, identifiers, numbers or tables can tokenize below 3 bytes per token. Such an input passes the estimate, the server keeps only part of it (measured: 1,026 of several thousand tokens at num_ctx 2048), the reported count is then low, so neither the post-check nor estimate_exceeded fires, and the answer is recorded from a shortened prompt. Options: flag a reported count far below the estimate (reported < estimate x a declared floor) as `suspected_truncation`, measure the server's keep length and flag a count equal to it, or keep a headroom fraction of num_ctx unused.


## Dispositions

Round 1, reviewed at agent-ix/sapho@2812aa1800068709943743661d947c4d1ad21692.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 2812aa1: prompt_eval_count is now a lower bound, a missing count is not an error (AC-4), and a cached-prefix count below the estimate never sets estimate_exceeded (AC-3). Measured behaviour cited (0.32.14 counts the whole prompt). |
| FND-002 | fixed | 2812aa1: Weights digest read from /api/show before every request; change gives ModelMismatch weights_changed (FR-052-AC-4). |

## New findings (disposition pass 2)

Reviewed at agent-ix/sapho@a34c54c8909d6f73a6a2258e4419fa2fa5b0af38.

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-004 | low | `max_bytes_per_token` 6.0 is the largest measured ratio, not a proven bound | spec/modules/ollama/functional/FR-050.md:33 |

### FND-004 (low, confidence low, other)

Unit: FR-050 at spec/modules/ollama/functional/FR-050.md:33. Related: FR-054.

> The byte estimate divides by the largest measured bytes per token, so it is a lower bound on the token count: when even the lower bound does not fit, the request is pointless and is not sent.

`max_bytes_per_token` 6.0 is the largest measured ratio, not a proven bound. Tokenizers merge long whitespace runs and repeated punctuation into single tokens of many bytes, so heavily indented or padded text can exceed 6 bytes per token, making the 'lower bound' estimate too high and refusing pre-send a prompt the server would accept. Impact is limited to inputs near the limit, and `below_estimate` records the evidence; say 'measured' rather than 'lower bound', or let the server refusal decide when the estimate is within a margin of num_ctx.


## Dispositions (round 2)

Round 2, reviewed at agent-ix/sapho@a34c54c8909d6f73a6a2258e4419fa2fa5b0af38.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-003 | fixed | a34c54c: Requests send `truncate: false` and `shift: false`; the server's refusal with n_prompt_tokens is the authority, the byte estimate is only a lower-bound pre-check (max_bytes_per_token 6.0). Coherent with sapho-dataset FR-010 too_large recording (four numbers). |
