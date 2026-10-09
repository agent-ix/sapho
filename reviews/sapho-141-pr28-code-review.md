---
id: SR-061
title: "Code and Rust review of Ollama external retag contract"
type: SpecReview
analysis: code-review
scope: "agent-ix/sapho@5be1e7061fe266e9951faa0dc551c4898c0460b9; crates/sapho-ollama/src/identity.rs, crates/sapho-ollama/src/embed.rs, crates/sapho-ollama/tests/extraction.rs, docs/ollama-provenance.md, spec/modules/ollama/functional/FR-052.md"
review_set: subset
---
# SR-061: External retag code review

## Summary

Ticket: SAPHO-141. Reviewed the complete PR diff, Rust implementation and loopback race test. Official Ollama OpenAPI GenerateResponse and EmbedResponse expose a model name without an answering weights digest.

## Verdict

FAIL at the reviewed SHA. The focused cargo test cannot compile. The changed test code also placed the intended post-retag show behavior in an unrelated test, leaving the new race test inconsistent with its assertion.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | Race fixture references an undefined variable in another test and cannot compile | crates/sapho-ollama/tests/extraction.rs:239 |
| FND-002 | high | AC-7 race fixture always serves digest A but asserts a later show returns B | crates/sapho-ollama/tests/extraction.rs:605-630 |

## Coverage

- FR-052-AC-7 (examined): scripted external retag between show and generate must retain A as observation and show that the response does not attest answering weights.
- FR-052-AC-1 through FR-052-AC-6 (context only): existing identity, raw exchange and changed-between-calls behavior.
- No applicable AssuranceProfile found. No CI workflow change, vendored artifact, production unsafe or new production panic surface in the diff.
- `cargo test -p sapho-ollama --test extraction external_retag_between_show_and_generate_leaves_only_a_pre_request_observation` exited 101 with E0425 at extraction.rs:239.

## Dispositions

| FND | outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
| FND-002 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
