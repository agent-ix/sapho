---
id: SR-059
title: "EARS analysis of FR-052 public lookup statement"
type: SpecReview
analysis: ears-conformance
scope: "agent-ix/sapho@4e983edb7393af5fa36feb84150c0da15da388d1; spec/modules/ollama/functional/FR-052.md"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-052"
    type: references
---
# SR-059: FR-052 EARS analysis

## Summary

Ticket: SAPHO-95. Reviewed the newly added ubiquitous behavior statement and AC-6 for EARS grammar, singularity, and testability.

## Verdict

PASS. The statement names the actor and observable lookup behavior; its follow-on sentence states the explicit distinction between unpinned public lookup and pinned generate/embed work.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | low | No findings (placeholder) | - |

## Coverage

- FR-052 behavior, public `Server::weights_digest` statement (examined): `Server::weights_digest` SHALL expose an unpinned lookup of the current weights digest through the configured server URL, headers and request bounds.
- FR-052-AC-6 (examined): A public `Server` lookup posts the named model through configured headers and returns a changed digest on a second call without pinning; an unknown model yields `Config` and `model_not_installed`.
