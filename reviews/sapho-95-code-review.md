---
id: SR-056
title: "Code and Rust review of public weights digest lookup"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho@4e983edb7393af5fa36feb84150c0da15da388d1; crates/sapho-ollama/src/identity.rs, crates/sapho-ollama/src/http.rs, crates/sapho-ollama/src/backend.rs, crates/sapho-ollama/src/embed.rs, crates/sapho-ollama/tests/weights_lookup.rs, spec/modules/ollama/functional/FR-052.md"
review_set: subset
---
# SR-056: Public weights digest code review

## Summary

Ticket: SAPHO-95. Reviewed the complete PR diff, Server transport and permit implementation, pinned generate/embed callers, loopback tests, and FR-052 criteria.

## Verdict

PASS for the changed behavior. The two new loopback tests and full `sapho-ollama` crate test suite pass in both feature lanes. Formatting and both Clippy lanes pass. Full `make ci` reaches an unrelated CLI peak memory test that cannot read `kern.clockrate` in the sandbox. That exact isolated test passes outside the sandbox (1 passed, 0 failed). The remaining deny, unsafe audit, docs, examples and spec gates pass independently.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | low | No findings (placeholder) | - |

## Coverage

- FR-052-AC-6 (examined): A public `Server` lookup posts the named model through configured headers and returns a changed digest on a second call without pinning; an unknown model yields `Config` and `model_not_installed`.
- FR-052-AC-4 (context only): When the scripted description changes its `FROM` blob between two calls, the second call returns `ModelMismatch` with `weights_changed` and no generate request is recorded for it; a response naming a different model yields `ModelMismatch` with `name_mismatch` and the raw exchange.
- No applicable AssuranceProfile was found. No source duplication, new gate, public private-data reference, unsafe code, or production panic surface was introduced.
