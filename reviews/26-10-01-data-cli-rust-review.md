---
id: SR-019
title: "Rust and code review of YAML/JSON data workflows"
type: SpecReview
analysis: code-review
scope: "sapho@762233e51d0853795c511834d036f566490687c4 versus main@0082a8dcca5e2e9d91968f32176e0177a99dc24b; all changed Cargo manifests/lockfile/deny.toml and CLAUDE.md; sapho-core data/lib/tests; sapho-graph spec/compiler/tests; sapho-runtime engine/operators; sapho-recording loader; complete sapho-cli, sapho-evidence and sapho-select source/tests; embedding facade and engine assembly scenarios; README, user/CLI/assessment guides, original examples, portable plugin/three skills, extension specs and matrix; unchanged CI gates examined"
review_set: subset
---
# SR-019: Rust and code review of data workflows

## Summary

Applied the repository conventions and rust-review checklist to the entire feature diff. The typed boundaries, offline replay and measured examples are sound; Git filter enumeration misses included local configuration, and deadline cleanup needs direct execution evidence.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | high | A repository can define a clean/process filter in a locally included config. `git config --local --get-regexp` omits includes by default, while diff reads them; the selector can execute that repository command despite its no-filter contract. Enumerate effective local includes before overriding filter commands. | crates/sapho-select/src/git.rs:147 |
| FND-002 | medium | The owned-child test proves kill/reap on output exhaustion, but not after a child starts and exceeds its deadline. A regression in deadline cleanup could satisfy all existing tagged tests. Add an observed child identity and deadline refusal/reaping test. | crates/sapho-select/src/tests.rs:401 |

## Analysis

No applicable AssuranceProfile exists. No copied dependency source/schema/private consumer artifact is introduced. Examples and plugin assets are original Sapho material; external schemas and dependency implementations are referenced at their authoritative homes. AGPL workspace inheritance and explicit self-crate license exceptions remain intact. CI workflows and both feature lanes are unchanged.

The core decoder uses a closed schema and duplicate-aware bounded JSON traversal. The constrained YAML visitor uses a stack-growing wrapper while retaining explicit depth/document/alias/anchor budgets. Exact anchor/alias LimitExceeded versus unsupported-form Config assertions pass. Record/list signatures, occurrence IDs, inherited sources, nested item/data ceilings and projections are tested through the actual compiler/runtime. No production panic, unsafe block, stub, script evaluator or string-derived error routing was found.

Evidence scoring separates Boolean agreement from Probability Brier and unsupported Degrees, validates provenance and prediction payloads, exposes denominators, and excludes held-out cases from tuning/export. Candidate indices refer to the original supplied list even when earlier candidates fail compilation. CLI acquisition/persistence surrounds asynchronous evaluation; Unix stdin polling does not change shared flags, named FIFOs refuse, and destinations are exclusively claimed. Actual examples exercise weighted reduction, threshold comparison, empty/file selection, recording and replay. Custom native inspection observes zero executions; backend capture/replay observes exact requests without live fallback. Stock live SDK preparation is feature gated and no live inference was used for this review.

Portable root skill discovery, frontmatter, repository-owned references and offline forward workflows have behavioral tests. The manifest was also validated against its authoritative remote schema in memory, without copying it. No package installation or publication is claimed.

## Validation

Scoped lint passed with all features. CLI acceptance passed with 9 default-lane tests and 8 all-feature tests; core 13, graph 7, evidence 4, selection 6 and root assembly filter 4 passed. Full pre-PR/pre-merge gates remain required after fixes. Optional independent semantic review is not claimed.

## Verdict

FAIL at the reviewed revision. Fix both findings before merge; preserve the findings and record their dispositions separately.

## Dispositions

| Finding | Outcome | Evidence |
|---|---|---|
| FND-001 | fixed 2a9c2e206a5f96b36fd9ac6f6eee2e32708e4556 | `--includes` enumerates effective local filter definitions. The included-config marker test failed on the reviewed source and passes after the fix; direct filters remain disabled. |
| FND-002 | fixed 2a9c2e206a5f96b36fd9ac6f6eee2e32708e4556 | A started child publishes its PID; acquisition returns Deadline and the reaped PID is absent. No elapsed-time threshold assertion. |

## Final Review Verdict

PASS after verifying the fix diff. Additional verification at 9d19ae25d2ed54463eab091f838b868670fa1bf6 exercises actual working-tree/staged/revision selectors through the stock CLI and the same typed graph boundary. Guide text now states stdin/file acquisition limits explicitly. Full gate results are recorded separately; no second independent reviewer is claimed.
