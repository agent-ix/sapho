---
id: SR-022
title: "Provider and CLI base review"
type: SpecReview
analysis: base
scope: "spec/modules/clm; spec/modules/cli/functional/FR-045.md; spec/modules/cli/functional/FR-046.md; spec/spec.md"
review_set: subset
---
## Summary

Reviewed the complete authorized provider/CLI change before implementation against owning core, runtime, recording and CLI contracts. Decisions is explicitly dependent on an unavailable official contract and does not block CLM or CLI.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No findings (placeholder) | - |

## Analysis

Request/response schema was checked against current authoritative CLM client, schema, engine and server. Boolean uses true/false, confidence retains provider meaning, usage includes billing units. Each behavior has explicit positive/error/boundary/offline criteria; tests are planned at actual transport/native seams. New IDs are globally unique. Package revision selection belongs to Cargo, not normative criteria. Peter explicitly requires source provenance, exact replay, pinned dependencies, Trace criteria and recorded review revisions; those instructions supersede the generic checklist prohibition against tracking/provenance. Existing recording identity is the product feature and is retained.

## Verification Plan

Translation and evidence: FR-043-AC-1..4; HTTP bounds/errors/concurrency: FR-044-AC-1..4; shared credentials, dispatch and provider-independent replay: FR-045-AC-1..3; process contracts/version/config: FR-046-AC-1..2. FR-047 uses inspection until contract availability. Computed test coverage will be measured before handoff; no implementation coverage is claimed at specification time.

## Request Translation Allocation

Pre-refactor review identified that CLM request translation duplicated the existing Jev translator. Both adapters now allocate identical request encoding to sapho-systemone, directly using SDK-owned wire types. No duplicate custom question/request schema remains; response decoding stays provider-specific because CLM usage and strict structural requirements differ from SDK decoding. Core has no sibling dependencies. Ordering is unchanged: shared codec precedes both adapters. The existing Jev public export refers to the moved implementation, with no fallback or compatibility reader.

## Verdict

PASS for the specification scope. This review preceded implementation; final code and execution evidence are recorded separately in SR-026 and SR-027.
