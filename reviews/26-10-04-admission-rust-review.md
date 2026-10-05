---
id: SR-034
title: "Request envelope Rust review"
type: SpecReview
analysis: base
scope: "FR-048; sapho-ollama lib.rs/bounds.rs"
review_set: subset
---
# Request envelope Rust review

## Summary

Self-review of exact source revision `285b339a0adba2313997a48b89d8b2fe2fec4cc1` using the Rust-review checklist and repository conventions. Reviewed complete typed/prompt/wire encoder accounting, checked overflow, placeholder shape validation, host-proof seam and unchanged byte/context limits. Worst-case wire escaping is explicit and may reject conservatively. Schema menus supplied by the host must dominate every possible actual menu; this API cannot prove a domain plan itself. Non-null probability maps violate the already-declared local wire schema and now cause typed InvalidAnswer while retaining HTTP200 raw bytes, service usage and one dispatch. Native distribution APIs are unchanged. Tests measure real encoders for Unicode/control/quote text and both explicit formats; loopback transport verifies immutable invalid-response retention without retry. No source-domain policy, model calls to a live service, runtime configuration change or dependency change.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | medium | Producer bounds do not establish paired EARS admission, runtime binding or production readiness without consumer wiring and executable should-fit/should-refuse tests. | FR-048 |

## Verification

Workspace all-feature offline tests PASS, including 15 provider tests; one subprocess-helper test ignored and exercised by its parent. Strict all-target/all-feature Clippy PASS; formatting and diff check PASS; FR-048 validation exit0 with existing advisory warnings.

## Verdict

PASS for this scoped producer candidate; CONDITIONAL for Qwen production. This is self-review, not an independent reviewer approval. Conservative valid-request envelopes do not guarantee valid model output or semantic correctness.

## Dispositions

FND-001: deferred to active consumer integration; live inference remains held. No merge is requested or performed.
