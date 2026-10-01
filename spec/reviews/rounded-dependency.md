---
id: SR-010
title: "Approximate distribution dependency review"
type: SpecReview
analysis: dependency
scope: "Focused approximate-distribution contract change in spec/spec.md and owning modified FR/TC artifacts"
review_set: subset
---
# Approximate distribution review

## Summary

FR-006 host binding and FR-005 validation enable FR-020 projections and FR-027/028 recording. Consumer FR-007/010 adopts this authoritative API after Sapho lands. Dependency order is acyclic; existing native/backend seams are sufficient. No new package, provider endpoint, schema copy or compatibility layer is required.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the focused specification before implementation. No accuracy or calibration claim.

## Dispositions

A design-review concern about projections exceeding one was resolved before coding by explicitly deriving complete-distribution normalization from retained raw mass. A consumer revalidation concern was resolved by retaining policy per Decision. Strict-only CLI behavior is explicit; embedding replay supplies its policy. No review finding remains open.
