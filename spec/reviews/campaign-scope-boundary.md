---
id: SR-028
title: "Campaign extraction scope-boundary review"
type: SpecReview
analysis: scope-boundary
scope: "spec/modules/campaign/spec.md"
review_set: subset
---
## Summary

Trusted compiled adapter extensions own domain tables and semantics. Generic crates contain no EARS quotas, Quire contracts, private campaign data or Git publication.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No unresolved design findings; implementation and evidence still required. | FR-049, FR-050, FR-051, FR-052 |
