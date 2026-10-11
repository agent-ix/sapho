---
id: SR-266
title: "Gap analysis of normalized SAPHO-166 Claude code"
type: SpecReview
analysis: gap-analysis
scope: "SAPHO-166 code c4f3e9ae6e0fc42c136fc4673fb174b4d6de7ae1; FR-088, FR-089, FR-090, IT-019"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-088"
    type: references
  - target: "ix://agent-ix/sapho/FR-089"
    type: references
  - target: "ix://agent-ix/sapho/FR-090"
    type: references
---
# SR-266: Gap analysis of normalized SAPHO-166 Claude code

## Summary

Read-only acceptance review at exact clean head `c4f3e9ae6e0fc42c136fc4673fb174b4d6de7ae1` against main `e32c4bb`. The strict computed matrix exits zero and tags all thirteen FR-088/089/090 criteria, but tags overstate executable acceptance. No build was run in this review.

## Verdict

**HOLD**. The advertised optional Claude-only CLI feature has a compile-gating defect, and several named IT-019 boundary cases have no direct executable assertions.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Two adjacent `#[cfg]` attributes on `credential` are conjunctive: the first excludes `claude` alone while `prepare_claude` calls the function. The public `sapho-cli --no-default-features --features claude` configuration therefore cannot compile. Replace with one four-provider cfg and check the isolated feature build. | `crates/sapho-cli/src/bindings.rs:352-359`; FR-090-AC-1; IT-019-SC-04 |
| FND-002 | medium | The FR-090-AC-1/2/3 tags in `tests/claude.rs` exercise metadata helpers and an injected registry, but no stock CLI `run` or `record` dispatch, required-only preparation, workspace environment read, missing key/feature refusal, or offline command without the Claude feature/key. The tagged tests do not demonstrate a provider/model-only binding swap executing the same graph. Add focused stock invocation fixtures including feature-free replay and poisoned credentials. | `crates/sapho-cli/tests/claude.rs:51-211`; FR-090-AC-1/2/3; IT-019-SC-01/04 |
| FND-003 | medium | Boundary tags do not assert several explicitly named cases: more than 16 options/levels, nonfinite/out-of-range answer values and extra answer ID, transport failure, native redirect refusal, invalid/empty workspace ID, 128-byte workspace header, and 400 when required workspace ID is absent. Add table-driven synthetic cases that assert error code, call count, and redaction at these boundaries. | `crates/sapho-claude/src/tests.rs`; FR-088-AC-3/4; FR-089-AC-1/3/4; IT-019-SC-02/03/04 |

## Covered evidence

- `quire matrix --strict --scope . --format tsv` exits zero; all thirteen criteria are tagged. `git diff --check e32c4bb...HEAD` exits zero.
- Existing tests capture a three-kind structured request, preserve actual model/usage/raw JSON, check Strict/Approximate mass, exercise 400/401/403/422/429/500, request and response byte ceilings, a permit ceiling, deadlines, and generic recording/replay.
- The fixed schema has no case-derived labels; `sapho-http::NativeTransport` disables redirects and retries and bounds streamed bodies. The stock binding metadata denies unknown fields. The new source does not alter Dataset serde/validate or add EARS annotation wiring.
- `docs/cli-guide.md` documents a separate authorized live smoke. No live Claude result or access receipt is present, so live acceptance remains pending rather than inferred from synthetic tests.

Plan completion: not assessed.

## Dispositions

- FND-001: fixed `83db42857fa59c2fc8f52ddc45da6ee2e7395b68`. The credential helper has one feature gate covering Jev, CLM, Decisions and Claude; the Claude-only build is a separate verification gate.
- FND-002: fixed `83db42857fa59c2fc8f52ddc45da6ee2e7395b68`. Stock run/record process tests exercise required and unused Claude bindings, credential and workspace refusal; feature-free replay uses poisoned host values; a same-graph two-model execution fixture checks binding selection. An authorized live Claude call remains a separate acceptance item.
- FND-003: fixed `83db42857fa59c2fc8f52ddc45da6ee2e7395b68`. New table and loopback cases cover option/level limits, numeric and answer-ID refusal, invalid workspace values, native redirect and transport failure with redacted errors.
