---
id: SR-035
title: "Local model pin Rust review"
type: SpecReview
analysis: base
scope: "FR-048 and sapho-ollama at 3c909aed2bf9781e6e59b998210267593d425677"
review_set: subset
---
# Local model pin Rust review

## Summary

Self-review using the Rust-review checklist. The optional pin is bound before
invocation, sealed under the same short mutex before any await, immutable across
calls and checked under the service capacity lease before generation. A drift
refusal retains the observed installed digest and zero generation dispatch.
No mutex spans an await. The additional inspection is metadata-only. Endpoint
validation continues to exclude credentials, queries, redirects and remote hosts.
No domain rules or schemas are vendored. No live model or service configuration
was accessed. The invocation-seal change closes a startup binding race found in
review; first-time binding after even a refused invocation now fails.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | medium | Installed alias inspection cannot atomically establish the weights served by a later generation; callers must retain this limit and observe model identity in raw responses. | FR-048 |

## Dispositions

FND-001: explicit API/spec limitation, not an enforceable service-instance claim.
Consumer checks stage transition and records requested options separately from
unavailable effective runtime options. No merge authorization implied.

## Verification

16 provider tests PASS; one subprocess helper ignored and exercised by its parent.
Strict all-target/all-feature provider Clippy PASS; formatting and diff check PASS.
Digest drift, shared capacity release and late first-time binding refuse without
generation. Existing capture/timeout/schema/usage tests pass unchanged.

## Verdict

PASS for the scoped producer candidate; consumer runtime readiness requires its
pinned executable tests. This is self-review, not independent approval.
