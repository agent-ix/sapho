---
id: SR-005
title: "Sapho first-version Rust review"
type: SpecReview
analysis: code-review
scope: "sapho@d5d880291eca4a83ed2fc9c11c7961241972eefe; all crates/** source/manifests, src/lib.rs, tests/**, Cargo.toml, Cargo.lock, rust-toolchain.toml, rustfmt.toml, clippy.toml, deny.toml, Makefile, .github/workflows/**, scripts/**, CLAUDE.md, AGENTS.md, .agent/rules/**, README.md, spec/** and project rights/legal controls"
review_set: subset
---
# SR-005: Sapho first-version Rust review

## Summary

Reviewed the complete initial implementation against Sapho's own Rust conventions
and the rust-review checklist. This is a local self-review; no independent review,
live-model quality, service integration or external CI execution is claimed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the specified first version after the recorded final gates. No unresolved
implementation finding remains in the reviewed source. Model accuracy and downstream
domain policy remain outside this review's acceptance claim.

## Review evidence

- Core validates typed values, source ranges, schema depth and exact port membership.
  Public scalar constructors refuse non-finite/out-of-range values. Error categories
  are exhaustive stable codes; request/model identities are explicit. Serde payloads
  reject unknown fields. No production unwrap, expect, caller-data index or unsafe
  block was found; doctest/test assertions use ordinary fail-loud test operations.
- Graph compilation is pure: every definition is checked, native signatures are
  captured without evaluation, cycles and incompatible operands are refused, and
  compiled fields are private with read-only accessors. Limits bound TOML, node count,
  projections and map nesting. Crate dependencies match the reviewed scope allocation.
- Runtime validates root inputs before work, guards yield explicit absence, native
  functions run through spawn_blocking, and pending model futures are dropped on failure
  or deadline. Counters apply across maps. Native cancellation remains cooperative.
  Count expansion precedes pair allocation; output/data accounting precedes dependents.
  Limits are work/data ceilings, not a memory sandbox for host-native implementations.
- Trace paths distinguish subgraph/item instances. Dependency paths and resolved guards
  are retained, and invalid normalized model answers retain request/response evidence.
  Threshold tests assert contributing answers, source spans and the exact cutoff.
- Boolean, Probability, Degree and Optional meanings remain distinct. Missing choice
  mass is a refusal, expected scores remain fractional, weights align by identity,
  empty reductions are configured, and coalescing preserves false/zero/empty text.
- Jev uses the authoritative SDK packages, ordered questions and explicit one-attempt
  options. Contract tests inject the SDK transport, validate observed wire requests,
  preserve partial distributions/model identity/usage, and check sanitized refusal codes.
  Recording locks are never held across await; file methods are explicit synchronous
  host actions. Exact replay has no live delegate and refuses ambiguous duplicate answers.
- Integration tests reach public APIs. The only doubles sit at native/backend/SDK
  transport seams. The synthetic consumer exercises extraction, repeated occurrences,
  earlier answer dependent questions, filtering, pairs and interpretation assembly.
  Deadline tests use virtual time and observable future-drop signals.
- CI retains the scaffold's manual invocation policy and covers both feature lanes,
  docs, unsafe checks, release build and all-feature supply-chain checking. No existing
  lane was dropped. Spec validation remains a local installed-tool gate.

## Gate results

Final verification passed. The final `make ci` invocation exited 0. Checks below
are in its execution order; `CARGO_TARGET_DIR=target` is exported by Make.

| Command | Exit | Observed result |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | Formatted workspace |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No Rust warnings |
| `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` | 0 | No Rust warnings |
| `cargo test --workspace --all-features` | 0 | 35 behavior tests and 1 compiled embedding example pass |
| `cargo test --workspace --no-default-features` | 0 | Same 35 tests and example pass; facade does not enable Jev |
| `cargo deny --all-features check` | 0 | Advisories, bans, licenses and sources pass |
| `bash scripts/check_unsafe_comments.sh` | 0 | Unsafe audit passes; workspace forbids unsafe |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` | 0 | All six crates documented |
| `quire validate --scope . "spec/**/*.md"` | 0 | All modular specifications and four spec reviews valid |
| `quire validate --scope . "reviews/**/*.md"` | 0 | Formal Rust review valid |
| `make build` | 0 | `cargo build --workspace --release --all-features` passes |

`quire coverage --scope . --json` also exits 0 with the traceability result below.
`df -h /` was checked before the full gates, with more than 270 GiB available.
The first `make ci` attempt exited 2 because its final review-document glob was
empty before this artifact existed; all preceding Rust checks passed. The final
invocation validates this artifact as well. Development test/lint failures were
corrected before the reviewed commit.

Supply-chain output has two unused license-allowance warnings from the scaffold's
permissive allowlist. Quire emits ambient catalog duplicate/advisory warnings; it
reports no Sapho validation errors. These do not change the observed exit codes.

The new-project rights preflight initially refused Cargo.lock's unrecognized suffix.
Cargo.lock was reviewed as Cargo-generated UTF-8 registry metadata with no protected
or workstation content. Re-running the installed checker with that single reviewed
suffix admitted passes all its remaining checks without changing the installed skill.
Canonical AGPL text, org CLA, contributor guidance and rights controls are present.
No dependency implementation, private research data, weights or model artifacts are
copied into this repository. All model tests use synthetic responses.

## Traceability limits

Installed Quire 0.33.0 exposes coverage rather than the newer `matrix --strict` command.
The source-grounded coverage report finds 87/87 functional criteria tagged across 35
behavior tests, with no untracked tests or unmatched criterion tags. Twelve stakeholder
validation rows are product goals and have no separate source-test tag claim. Passing
criteria tags alone do not prove an oracle; their assertions were also inspected.
Two similarity suspicions concern test binding constructors, not an expected-result
oracle copied from production, and do not establish a tautology.

## Dispositions

No real findings required disposition. Corrections made during development are present
in the reviewed commit: immediate model-failure cancellation, empty-weight membership
checks, read-only compiled fields, validated distribution-state reporting, scoped trace
dependency paths and retained guards. No exception or compatibility layer was introduced.
