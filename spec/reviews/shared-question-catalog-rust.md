---
id: SR-037
title: "Shared question catalog Rust review"
type: SpecReview
---
# SR-037: Shared question catalog Rust review

## Summary

Formal Rust self-review of f015b8411f97b265682e69531a446fe386ee7226 against Sapho conventions and dev-tools rust-review. Examined complete catalog/prompt/bounds/lib changes, associated tests and FR-048; all other workspace crates exercised by regression gates. CI workflow diff is empty. Not independent approval or real-model validation.

## Findings

| ID | Severity | Summary | Refs | Escape Cause |
|---|---|---|---|---|
| FND-001 | low | No unresolved implementation findings in the examined generic codec increment. | FR-048 | correct-requirement-no-evidence |

## Review evidence

All three Question variants retain exact instruction bytes, menu/description strings, ordered IDs and spaces. Only byte-identical newline-inclusive fragments/spaces share. Typed serde and checked indices/counters own wire construction; no domain syntax or source parser exists in the codec. Separate catalog and typed JSON bounds are required; actual complete request measurement still guards dispatch. Templates carry explicit host-proof assumptions. The single-line bound pays for every raw string and checked reference/envelope overhead. Existing encodings/defaults remain. Complete typed ModelRequest, raw response, usage and immutable transport capture remain unchanged.

cargo test --offline -j2 --workspace --all-features: PASS. cargo test --offline -j2 --workspace --no-default-features: PASS. Twenty Ollama tests PASS, one subprocess helper intentionally ignored by direct harness; its parent exercises it. Exact dictionary roundtrip, Unicode/control/delimiter/newline cases, separately bounded shapes, real loopback dispatch and one-byte-over refusal exercise the encoder. Both strict workspace Clippy feature lanes: PASS. Strict rustdoc, unsafe-comment audit and diff check: PASS. quire spec validation: PASS with module-registration warnings. cargo deny --offline --workspace --all-features --locked check exits 0, reports advisories/bans/licenses/sources OK, but emits existing wildcard warnings and an unresolved-workspace-dependency tool diagnostic for ix-cli-kit; not a diagnostic-clean claim.

## Dispositions

FND-001 accepted-no-change. Downstream paired admission and actual Qwen execution are separate uncompleted evidence.

## Verdict

Generic provider increment is reviewed for downstream offline integration; no model-quality or campaign-completion claim.
