---
id: SR-167
title: "SAPHO-16 normalized Decisions acceptance gap review"
type: SpecReview
analysis: gap-analysis
scope: "SAPHO-16 FR-047/085/086/087, IT-018, code PR #51 at 6ef97ef72a9a6d6104aa863789357aee68d76476"
review_set: subset
relationships:
  - { target: "ix://agent-ix/sapho/FR-085", type: references }
---
# SR-167: SAPHO-16 normalized Decisions acceptance gap review

## Summary

Read-only planless audit of the normalized SAPHO-16 Decisions diff against merged main. The real adapter, bounded transport, provider registry, synthetic recording/replay and fifteen target criterion tags are present. Some IT-018 transport and refusal cases lack executable evidence. Live service acceptance remains blocked by the previously observed 429 insufficient-quota response; this audit made no live call.

## Verdict

**CONDITIONAL / HOLD** — close the concrete synthetic test gaps and re-review before code merge. This is not a finding that the production transport itself fails those cases.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | FR-086-AC-3/IT-018-SC-05 tags cover send stall, cancellation, advertised over-limit length and chunked overflow, but no separate permit-wait deadline, streamed-body stall deadline or falsely small Content-Length fixture. | FR-086-AC-3, IT-018-SC-05 |
| FND-002 | medium | Reserved-key malformed shapes are tested through the wire encoder, but FR-085-AC-6 also requires an adapter-level zero-transport-call assertion for those exact two shapes. | FR-085-AC-6, IT-018-SC-02 |
| FND-003 | medium | IT-018-SC-05 specifies a quota-shaped 429 body and sentinel diagnostics for key, input, body, header and URL; current fixed-status tests use a generic body and check only its sentinel. | FR-086-AC-4, IT-018-SC-05 |

## Coverage

- Reconciliation: `quire matrix --scope . --format json`, exit 0; fifteen target FR criteria tagged, none untagged. Matrix is syntax traceability, while the findings identify cases not exercised by the tagged tests.
- Plan completion: not assessed
- Criteria: tagged 15; untagged 0 in FR-047-AC-2 and FR-085/086/087.
- Untraced behaviors / stubs: no stub adapter or Responses/Chat fallback found in the changed source.
- Semantic review: targeted source/test comparison for the requested acceptance checks; no live provider verification.
- Dataset boundary: no Dataset source or serde/validate file changed in `git diff 8878eab...HEAD`.
- Source evidence: `crates/sapho-decisions/src/lib.rs`, `wire.rs`, `tests.rs`; `crates/sapho-cli/src/bindings.rs`, `credentials.rs`, `tests/decisions.rs`, `tests/providers.rs`; `spec/modules/decisions/integration/IT-018.md`.
- `git diff --check 8878eab...HEAD`: exit 0. Builds and tests were not rerun in this read-only review.

## Dispositions

- FND-001: fixed `d6f1572`. A permit-wait fixture asserts one dispatch under a full queue, a native HTTP fixture stalls after headers, and a conflicting small advertised length with a larger chunked body is bounded. The shared transport applies the body byte ceiling while the shared admission gate retains the complete deadline.
- FND-002: fixed `d6f1572`. Both reserved-key malformed state shapes now pass through `DecisionsBackend::infer`, assert typed refusal, and verify zero transport calls.
- FND-003: fixed `d6f1572`. A loopback 429 quota-shaped response carries key, input, body, header and URL sentinels; diagnostics contain only the typed code and HTTP status.
