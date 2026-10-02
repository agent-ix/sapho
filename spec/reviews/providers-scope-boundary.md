---
id: SR-024
title: "Provider and CLI scope-boundary review"
type: SpecReview
analysis: scope-boundary
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

| Requirement | Owner | Class |
|-------------|-------|-------|
| FR-043 | sapho-clm | infrastructure |
| FR-044 | sapho-clm | infrastructure |
| FR-045 | sapho-cli | cross-cutting |
| FR-046 | sapho-cli | cross-cutting |
| FR-047 | provider specification intake | infrastructure |
| StR-011 | Sapho host boundary | cross-cutting |

Core owns typed usage/answers; runtime owns policy validation; recording owns exact matching; CLM owns wire/HTTP; CLI owns synchronous secret acquisition and process streams. CLM service availability, actual model behavior and OS credential availability are assumed host dependencies. Adapter-to-wire behavior is guaranteed by synthetic transport contract tests (IT-006); provider quality is not claimed. ix-cli-kit implementation is consumed directly, never copied. Decisions contract is an external unavailable dependency, not assumed data. No downstream source/data or deployments enter scope.

## Request Translation Allocation

Pre-refactor review identified that CLM request translation duplicated the existing Jev translator. Both adapters now allocate identical request encoding to sapho-systemone, directly using SDK-owned wire types. No duplicate custom question/request schema remains; response decoding stays provider-specific because CLM usage and strict structural requirements differ from SDK decoding. Core has no sibling dependencies. Ordering is unchanged: shared codec precedes both adapters. The existing Jev public export refers to the moved implementation, with no fallback or compatibility reader.
