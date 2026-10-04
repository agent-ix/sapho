---
id: SR-029
title: "Campaign extraction dependency review"
type: SpecReview
analysis: dependency
scope: "spec/modules/campaign/spec.md"
review_set: subset
---
## Summary

Optional facade features isolate storage/TUI dependencies. The existing core/runtime/recording contracts remain authoritative; no reverse CLI or Quire dependency and no additional DAG engine.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No unresolved design findings; implementation and evidence still required. | FR-049, FR-050, FR-051, FR-052 |
