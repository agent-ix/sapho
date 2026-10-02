---
id: SR-025
title: "Provider and CLI dependency review"
type: SpecReview
analysis: dependency
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

| Requirement | Class | Prerequisites |
|-------------|-------|---------------|
| FR-043 | Feature | existing core ModelBackend; additive usage evidence |
| FR-044 | Feature | FR-043; asynchronous HTTP transport |
| FR-045 | Feature | FR-044; existing ix-cli-kit secrets |
| FR-046 | Feature | existing ix-cli-kit foundations |
| FR-047 | Enablement | external official Decisions preview contract |
| StR-011 | Feature | FR-043, FR-045, FR-046 |

Topological sequence: core usage and CLM wire → bounded HTTP → CLI provider/secret dispatch → end-to-end capture/replay; shared CLI foundations can be developed independently. All dependencies exist except the Decisions contract; no cyclic prerequisites or shared checkout edits are required. Downstream consumers receive additive Usage API coordination before implementation. Decisions integration requires a separately reviewed provider contract once available.

## Request Translation Allocation

Pre-refactor review identified that CLM request translation duplicated the existing Jev translator. Both adapters now allocate identical request encoding to sapho-systemone, directly using SDK-owned wire types. No duplicate custom question/request schema remains; response decoding stays provider-specific because CLM usage and strict structural requirements differ from SDK decoding. Core has no sibling dependencies. Ordering is unchanged: shared codec precedes both adapters. The existing Jev public export refers to the moved implementation, with no fallback or compatibility reader.
