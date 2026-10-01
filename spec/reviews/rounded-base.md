---
id: SR-007
title: "Approximate distribution base review"
type: SpecReview
analysis: base
scope: "Focused approximate-distribution contract change in spec/spec.md and owning modified FR/TC artifacts"
review_set: subset
---
# Approximate distribution review

## Summary

Spec/TC IDs and links remain stable. New AC4 clauses cover policy bounds, complete0.99/1.01, Strict/gross/zero/partial refusal, raw evidence, derived projection and replay policy mismatch. Existing criteria remain valid. No required assurance profile applies.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the focused specification before implementation. No accuracy or calibration claim.

## Dispositions

A design-review concern about projections exceeding one was resolved before coding by explicitly deriving complete-distribution normalization from retained raw mass. A consumer revalidation concern was resolved by retaining policy per Decision. Strict-only CLI behavior is explicit; embedding replay supplies its policy. No review finding remains open.
