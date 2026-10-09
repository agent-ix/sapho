---
id: SR-060
title: "Integrity review of FR-052 public lookup extension"
type: SpecReview
analysis: integrity
scope: "agent-ix/sapho@4e983edb7393af5fa36feb84150c0da15da388d1; spec/modules/ollama/functional/FR-052.md"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-052"
    type: references
---
# SR-060: FR-052 integrity review

## Summary

Ticket: SAPHO-95. Checked the new unpinned lookup obligation against existing per-binding pinning, model identity, error handling, and verification links.

## Verdict

PASS. Public lookup and generate/embed behavior have separate call paths and compatible requirements. The new criterion is observable and tagged by loopback tests.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | low | No findings (placeholder) | - |

## Coverage

- FR-052 public lookup statement (examined): `Server::weights_digest` SHALL expose an unpinned lookup of the current weights digest through the configured server URL, headers and request bounds.
- FR-052-AC-6 (examined): A public `Server` lookup posts the named model through configured headers and returns a changed digest on a second call without pinning; an unknown model yields `Config` and `model_not_installed`.
- FR-052-AC-4 (context only): the adapter refuses changed weights within a pinned generate/embed binding.
