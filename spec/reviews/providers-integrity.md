---
id: SR-023
title: "Provider and CLI integrity review"
type: SpecReview
analysis: integrity
scope: "spec/modules/clm; spec/modules/cli/functional/FR-045.md; spec/modules/cli/functional/FR-046.md; spec/spec.md"
review_set: subset
---
## Summary

Reviewed the complete authorized provider/CLI change before implementation against owning core, runtime, recording and CLI contracts. Decisions is explicitly dependent on an unavailable official contract and does not block CLM or CLI.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No unresolved findings in the reviewed change scope. | [CLM](../modules/clm/spec.md) |

## Analysis

US-011 → FR-043..047 → StR-011 → declared Test/Inspection methods. No conflicting constraints with existing strict/approximate policy or exact replay. Native secret failures are explicit; optional CLM auth distinguishes absence from locked/unavailable. HTTP loops have no retries/redirects; timeout includes concurrency queue. Unknown schemas refuse. The source request permits plain structured state, while Sapho retains its narrower Record input contract. No dependency is stubbed. Existing core/runtime limits bound nested work and aggregate concurrent model calls. Usage is additive provider evidence, not output-token reinterpretation.

## Dispositions

Pre-review refinement fixed guessed replay-provider identity, clarified timeout queue scope and credential-feature ordering, and replaced an assumed shared version formatter with the actual shared Agreement mechanism. No unresolved conflict remains.

## Request Translation Allocation

Pre-refactor review identified that CLM request translation duplicated the existing Jev translator. Both adapters now allocate identical request encoding to sapho-systemone, directly using SDK-owned wire types. No duplicate custom question/request schema remains; response decoding stays provider-specific because CLM usage and strict structural requirements differ from SDK decoding. Core has no sibling dependencies. Ordering is unchanged: shared codec precedes both adapters. The existing Jev public export refers to the moved implementation, with no fallback or compatibility reader.
