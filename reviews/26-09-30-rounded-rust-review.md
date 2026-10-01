---
id: SR-011
title: "Approximate-distribution Rust idioms and coverage review"
type: SpecReview
analysis: code-review
scope: "sapho@cfb659fd3ce833ec1ebe3d513a52de4d4bd9609b; complete focused PR diff; files: Cargo.toml, README.md, crates/sapho-core/src/distribution.rs, crates/sapho-core/src/lib.rs, crates/sapho-core/src/model.rs, crates/sapho-core/src/ports.rs, crates/sapho-core/src/tests.rs, crates/sapho-jev/src/tests.rs, crates/sapho-runtime/src/engine.rs, deny.toml, tests/common/mod.rs, tests/scenarios/recording.rs; owning changed spec/TC/review artifacts and committed repo conventions; Sapho/consumer boundary source also read"
review_set: subset
---
# Formal Rust and code review

## Summary

Applied /rust-review before design and again formally to this exact committed source diff. This is a source-grounded self-review. Repository conventions govern the review. CI workflow diff was checked first: no workflow lane change. DistributionPolicy is a closed typed enum, with finite unit-range allowance validated before binding/request use and capped at0.05. A struct Strict variant rejects unknown tagged fields. Raw Answer/ModelResponse never changes; bounded positive complete mass permits derived normalization without division by zero. Partial/unavailable answers preserve existing semantics. Deterministic maps and typed errors/diagnostic context remain intact. No unsafe, new production panic/indexing, blocking network/native work, locks across await, implicit retries, copied dependency/private fixture or compatibility reader. serde_json float_roundtrip ensures exact representable values survive recording load. Every new test binds owning AC tags and asserts behavior across the real core/SDK transport/runtime/recording seams; fixtures are original synthetic data. The consumer retains policy per Decision and revalidates serialized interpretations under it; Strict-only CLI and policy-sensitive replay remain explicit. No model correctness claim is inferred.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for idioms, smells and focused acceptance-backed behavior. Adequate for the bounded distribution interpretation change; this does not establish semantic model accuracy, calibration or independent review.

## Dispositions

The original Sapho SR-006/FND-001 strict-mass boundary finding is fixed by the explicit policy/raw-aware projection implementation at Sapho80d04b27c267a5c15ba3312aba4cfe89445eb2fd and consumerdf37a309bd96579799e4d39c66390e0b04879f82. The original assessment is preserved unchanged. During development, meaningful tests found tagged-unit unknown-field acceptance and loss of exact f64 values on JSON reload; struct Strict and float_roundtrip corrected them before this reviewed source commit. No waivers or compatibility layers.

## Verification

Scoped core/Jev/recording tests, consumer full-layer approximate/replay test and workspace all-feature Clippy passed. Full gates are recorded in the companion verification artifact before publication. Coverage reconciles 90/90 functional acceptance criteria, no unbacked rows/status lies/untracked symbols; test assertions were read, not merely counted.
