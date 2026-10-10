---
id: SR-066
title: "SAPHO-16 verified Decisions contract review"
type: SpecReview
analysis: base
scope: "FR-047, FR-085 through FR-087 and IT-018 against official public beta docs and current origin/main"
review_set: subset
---
# SR-066: SAPHO-16 Decisions contract review

## Summary

On 2026-10-10, reviewed the [official Decisions guide](https://developers.openai.com/api/docs/guides/decisions) and [create reference](https://developers.openai.com/api/reference/resources/decisions/methods/create), current Sapho core ModelRequest/ModelResponse/Answers/Usage, CLI binding/credential preparation, CLM bounded transport and recording/replay on origin/main `e77b81eddd90b606561f2ed606cde74f133ea937`. The official pages establish public beta POST `/v1/decisions`, bearer auth, `gpt-6-luna`, text or user-message input, predicate/choice/score questions, ordered answers including per-question refusal, model and usage. This is document and source inspection, not live service acceptance. The planner reported that a prior minimal use of the available session key returned 429 `insufficient_quota`/`credit_balance_exhausted`; this review did not repeat the call, and quota/access is unresolved.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | The earlier launch announcement did not establish a wire contract, but official public beta guide/reference now do. | FR-047, FR-085, FR-086 |
| FND-002 | medium | The guide says image inputs must be inline base64 data URLs; the create reference also lists public HTTP(S) image URLs. | FR-085, IT-018 |
| FND-003 | high | Core Answer has no refusal variant; accepting answered siblings would make incomplete Answers appear valid. | FR-085, IT-018 |
| FND-004 | high | Core Record state and Questions do not directly equal Decisions input/question wire types. | FR-085, IT-018 |
| FND-005 | medium | Core Usage lacks cache/reasoning/total fields, while Decisions reports them. | FR-085, IT-018 |
| FND-006 | high | A live provider could expose secrets, exceed limits or silently fall back to a different endpoint. | FR-086, FR-087, IT-018 |
| FND-007 | high | Official schema availability does not prove account quota or live model behavior. | FR-047, IT-018 |
| FND-008 | high | The image envelope trigger was ambiguous for a Record containing `decisions_input` with extra fields or an ordinary Text value. | FR-085, IT-018 |

## Dispositions

FND-008: any top-level `decisions_input` presence reserves the envelope; only the exact one-field and exact inner type shape enters image mode. Every other presence refuses `InvalidValue` before transport. Records without the key keep ordinary compact-JSON mapping. FR-085-AC-6 and IT-018-SC-02 exercise the two ambiguous shapes and no-send behavior.

## Verdict

Ready for planner review of a specification-only Decisions adapter. `quire validate --scope . 'spec/**/*.md'` exited 0 with only installed catalog diagnostics; `quire matrix --scope .` exited 0 and shows all seventeen FR-047/085/086/087 ACs without implementation tags; `git diff --check` exited 0. FR-047 is updated from its prior documentation-blocked status; no Rust adapter, provider registry, tracker state, Dataset contract or EARS source is changed in this PR. Beta changes require renewed source review before implementation.
