---
id: SR-015
title: "CLI and YAML/JSON base review"
type: SpecReview
analysis: base
scope: "sapho@7d04780960a0404f64b9c7560fc4a998cac60842; full authorized CLI/format extension spec diff: spec/spec.md, FR-007/TC-007/IT-001, FR-030..FR-042 and owning TC/US/StR/module indexes, NFR-005, docs/yaml-authoring-assessment.md; existing owning core/graph/runtime/logic/recording/Jev contracts read for consistency"
review_set: subset
---
# SR-015: CLI and YAML/JSON base review

## Summary

Reviewed artifact structure, criterion observability, errors, unhappy paths, planned verification, IDs and links against the fetched catalog and base checklist. Replay binding inference needs a deterministic refusal rule.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | medium | FR-035 does not define replay binding inference when one backend name has several recorded models/policies or when no exchange supplies a required name. The host could choose the wrong binding or imply a live fallback. | FR-035 |

## Analysis

The modified loader preserves the typed GraphSpec contract and declares strict parsing/order/string semantics. FR-030..042 each have observable criteria and a corresponding TC. Existing extension/error/limit contracts are retained. CLI exit semantics distinguish evaluated findings from execution refusal; empty/unscored measurement sets do not fabricate success metrics. Skills have explicit caller authorization and local evidence boundaries. The replay inference edge case must be specified; synthetic offline/backend-seam tests will later verify it. No new assurance profile applies.

## Verdict

Revise the identified contracts before implementation. This is a pre-implementation review: TC scenarios are planned verification, not claims of passing Rust tests. Code/test trace coverage is checked at implementation handoff.

## Dispositions

FND-001: fixed 20f522b9246b3281eec964285f3a7b17380dc5b9; owning contract updated before implementation.

## Final Verdict

Pass for implementation after the documented fixes. All four selected review lenses have validated artifacts; runtime/test evidence remains a later implementation obligation.
