---
id: SR-264
title: "SAPHO-31 normalized code acceptance gap review"
type: SpecReview
analysis: gap-analysis
scope: "agent-ix/sapho@e8fa55ee97773f62714128adc126296d38d099fa; FR-072, FR-073, IT-013 and changed source/tests"
review_set: subset
relationships:
  - { target: "ix://agent-ix/sapho/FR-072", type: references }
  - { target: "ix://agent-ix/sapho/FR-073", type: references }
  - { target: "ix://agent-ix/sapho/IT-013", type: references }
---
# SAPHO-31 normalized code acceptance gap review

## Summary

Planless review of PR #47's normalized code against the merged specification. All twelve FR-072/073 criteria have computed test binders, and the previous credential-leak and changed-request replay gaps remain fixed. Two portions of tagged criteria still lack executable assertions.

## Verdict

CONDITIONAL — test coverage of FR-072-AC-3 and FR-072-AC-6 is incomplete; HOLD the merge gate pending focused fixtures.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | medium | The tagged forbidden-field test checks `endpoint`, `credential` and `api_key` in binding metadata, but no test injects them into a graph Ask/provider position. The graph `Operation::Ask` has `deny_unknown_fields`, so this appears implemented but FR-072-AC-3 and IT-013-SC-04 require a refusal fixture. | FR-072-AC-3, IT-013-SC-04; `crates/sapho-cli/tests/services.rs:476`, `crates/sapho-graph/src/spec.rs:96` |
| FND-002 | medium | Replay with a removed service file proves one offline path, but no tagged test proves `validate`, `inspect` or replay-backed `measure`/`tune` ignore a missing or malformed service file and avoid credential lookup. The tagged FR-072-AC-6 test covers only an unused live entry and omitted-model behavior. | FR-072-AC-6; `crates/sapho-cli/tests/services.rs:207`, `crates/sapho-cli/tests/services.rs:279`, `crates/sapho-cli/src/command.rs:105` |

## Coverage

- Reconciliation: `quire matrix --scope . --format json`; Quire CLI 0.36.2/engine 0.50.2; no run evidence read. FR-072-AC-1..7 and FR-073-AC-1..5: tagged 12, untagged 0.
- Plan completion: not assessed
- Prior gaps: injected secret is absent from recorded/report bytes; same-binding changed request yields `replay_miss` (`services.rs:314-374`, `services.rs:225-239`).
- Other examined paths: two/three fake services, distinct BackendIds, per-service limits including exact maxima and over-max refusal, response/deadline/concurrency tests, legacy Jev/CLM checks, unchanged Dataset and EARS source files. No new unowned production behavior or stubs found.
- Semantic review: limited to the ticket's stated criteria as requested.
- `git diff --check 9179323...HEAD`: exit 0. No build or test was run in this review.

## Dispositions

- FND-001: fixed `5a11209a389c17d3bac555be17d338bb2417f26b`. `graph_ask_refuses_provider_endpoint_and_credential_fields` checks all three forbidden fields through stock CLI validation.
- FND-002: fixed `5a11209a389c17d3bac555be17d338bb2417f26b`. `offline_commands_ignore_missing_and_malformed_service_files` checks validation, inspection, replay, measured scoring, and tune with missing and malformed service files. Focused `cargo test -p sapho-cli --all-features --test services` passed 11/11 at that commit.
