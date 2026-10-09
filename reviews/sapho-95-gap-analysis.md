---
id: SR-057
title: "Gap analysis of public weights digest lookup"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho@4e983edb7393af5fa36feb84150c0da15da388d1; spec/modules/ollama/functional/FR-052.md, crates/sapho-ollama/src/identity.rs, crates/sapho-ollama/tests/weights_lookup.rs, crates/sapho-ollama/tests/extraction.rs"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-052"
    type: references
---
# SR-057: Public weights digest gap analysis

## Summary

Ticket: SAPHO-95. The computed strict matrix tags all six FR-052 criteria; the new public behavior is owned by AC-6 and exercised by two loopback tests.

## Verdict

PASS for the reviewed requirement and implementation scope. The tests exercise actual Server HTTP behavior and assert both returned digests, the configured header and model request, unknown-model error, and request counts.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | low | No findings (placeholder) | - |

## Coverage

Plan completion: not assessed

- FR-052-AC-1 through FR-052-AC-5 (examined): existing extraction and identity criteria have trace bindings in the computed matrix.
- FR-052-AC-6 (examined): A public `Server` lookup posts the named model through configured headers and returns a changed digest on a second call without pinning; an unknown model yields `Config` and `model_not_installed`.
- No new unowned behavior, source stub, test stub, or coverage suppression was found in the PR. Optional semantic review was not requested.
