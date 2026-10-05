---
id: SR-036
title: "Shared question catalog design review"
type: SpecReview
---
# SR-036: Shared question catalog design review

## Summary

FR-048 catalog encoding increment on parent 4547d22c94445cf99facf3366005f5fe1e141eda; pre-implementation self-review using base, integrity, scope, dependency and Rust checklists. No independent approval or runtime claim.

## Findings

| ID | Severity | Summary | Refs | Escape Cause |
|---|---|---|---|---|
| FND-001 | high | Reusing a typed question byte bound for the catalog could admit unbounded reference overhead. Require an explicit separately proved catalog bound and actual per-call guard. | FR-048 | missing-requirement |
| FND-002 | high | Sharing source-shaped strings by similarity could alter meaning. Only byte-identical instruction fragments and typed spaces may share; roundtrip all three variants and delimiter/Unicode/newline cases. | FR-048 | missing-requirement |

## Dispositions

Both requirements incorporated before implementation. Generic provider owns encoding only; native/host own template and source bounds. No model calls, retry, cap changes or source truncation. Checked indices and bounded validation precede rendering; codecs use serde typed structs and exhaustive enums. Historical receipts and formats remain unchanged.

## Verdict

Proceed to offline implementation; runnable downstream admission remains unproved until integrated tests and real preflight.
