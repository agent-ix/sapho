---
id: SR-008
title: "Approximate distribution integrity review"
type: SpecReview
analysis: integrity
scope: "Focused approximate-distribution contract change in spec/spec.md and owning modified FR/TC artifacts"
review_set: subset
---
# Approximate distribution review

## Summary

Strict numerical roundoff and host approximation are distinct. Raw Answer/ModelResponse are immutable evidence; normalization exists only as a derived complete-distribution projection. Positive small bound and positive mass prevent division by zero or unbounded adjustment. Partial/unavailable semantics and model confidence/score remain independent. No retries or network-definition discovery added.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the focused specification before implementation. No accuracy or calibration claim.

## Dispositions

A design-review concern about projections exceeding one was resolved before coding by explicitly deriving complete-distribution normalization from retained raw mass. A consumer revalidation concern was resolved by retaining policy per Decision. Strict-only CLI behavior is explicit; embedding replay supplies its policy. No review finding remains open.

## Immediate token-boundary addendum before implementation

Observed HTTP400 max_tokens_exceeded after the rounded roles stage exposes duplicate model-context question metadata. FR-010 now defines a compact typed context carrying unchanged full source/context, spans, raw decisions and plan IDs; all original definitions remain in native/results/trace and the current Questions port. Expert context includes assembled findings. An AC binds behavior at the backend seam, with full pipeline/expert/replay tests. Sapho FR-026 assigns HTTP400 to ServiceValidation and tests it at the SDK transport seam. No source/context truncation, batching/retry policy, new dependency or compatibility reader is added. Existing integrity/scope/dependency allocation is unchanged: consumer owns prompt projection; Sapho owns service classification. Reviewed the amended contract before coding; no unresolved finding.
