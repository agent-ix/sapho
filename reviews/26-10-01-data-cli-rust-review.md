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

## Gate Findings

The first complete gate passed both lint/test lanes but stopped at the supply-chain check. These additional findings were measured during the same review's fix round, not hidden by a passing test count.

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-003 | high | The root package is not a virtual workspace. Cargo-deny without --workspace roots only the facade and omits evidence/selection/CLI dependencies; its unmatched self-crate exceptions expose that omitted gate coverage. | Makefile:23, .github/workflows/ci.yml:38 |
| FND-004 | medium | The stack-growth dependency brings a build-only archive writer with Apache-2.0 WITH LLVM-exception, which the original allowlist does not recognize. The first full gate refuses this exact expression. | Cargo.lock:25, deny.toml:1 |

The authoritative cargo-deny help confirms --workspace root selection. The authoritative ar_archive_writer 0.5.3 LICENSE.txt was read in the Cargo cache: its exceptions add redistribution permissions to Apache-2.0. The policy change is restricted to this examined build-only crate/version, preserving the global allowlist, advisory checks and all Sapho AGPL licensing. No dependency license file is copied into this repository.

The strengthened workspace check additionally found Windows capability dependency winx 0.36.4 with the same Apache/LLVM expression. Its authoritative LICENSE was read locally; its permission is likewise restricted to that exact dependency version. Cross-platform dependencies remain in the gate even on this Linux host.

## Complete-Stream Parser Finding

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-005 | high | The dependency's single-document helper deliberately ignores malformed content after an explicit `...` end marker. `outputs: {}\n...\n[invalid` is accepted as an empty graph, violating strict complete configuration refusal. | crates/sapho-graph/src/spec.rs:222 |

The regression test failed with an observed Ok graph before remediation. Use the dependency's complete-stream parser with the existing one-document budget and the same stack-safe typeless visitor; no dependency implementation or legacy reader is copied into Sapho.

## Gate-Finding Dispositions

| Finding | Outcome | Evidence |
|---|---|---|
| FND-003 | fixed cc67c9115f4f63dfc750135312a4dcabb2e1a167 | Local and CI cargo-deny now use --workspace --all-features --locked; all new workspace crates and cross-platform dependencies are checked. No feature lane removed. |
| FND-004 | fixed cc67c9115f4f63dfc750135312a4dcabb2e1a167 | Two examined Apache/LLVM dependency versions have narrow license permissions; workspace advisories, bans, licenses and sources checks pass. Sapho remains AGPL. |
| FND-005 | fixed 34193c29109943f2e244faca463a4bef81ec5252 | Complete-stream parsing rejects malformed trailing content as Config and a second YAML document as LimitExceeded. Anchor/alias, depth, duplicate, merge, tag and representation-equivalence tests still pass. |

## Final Fix-Round Verdict

PASS: all five recorded findings have verified fixes. The final full gates must still be completed and their results retained before PR/merge; the failed initial license gate is not counted as a pass.

## Documentation Gate Finding

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-006 | medium | The CLI binary name sapho collides with the existing facade library's generated documentation path. Cargo reports the collision even though the full gate exits successfully; one artifact can overwrite the other. Disable binary documentation and retain both public library documentation sets. | crates/sapho-cli/Cargo.toml:12 |

The replay specification's mixed-modal warning was also removed by stating exact-request threshold reuse directly, with no contract change.

## Documentation Disposition

FND-006: fixed bd0152aedfd79263cffcd76e4f87024b864ea2b6. Cargo docs now generate the facade and sapho-cli public library without the duplicate binary artifact; scoped workspace documentation exits 0 with no collision warning. No public API documentation or lint/test lane was removed.

## Executed Full Gate

`make ci` exited 0 at 9609f6711344362613b7f264815ce268e7a14703: all-feature lane 68 tests plus one doctest; no-default-feature lane 69 tests plus one doctest; both lint lanes; format check; complete-workspace locked advisory/bans/license/source audit; unsafe-comment audit; workspace Rust docs; specification and review validation. The initial license refusal was fixed and is not counted as a pass. A subsequent documentation collision warning was resolved as recorded above. The final pre-PR and pre-merge gate runs include that configuration fix and the verified matrix markers.

All six findings have verified dispositions. Final code/Rust verdict: PASS. No live inference, installation, training or downstream consumer migration is claimed.
