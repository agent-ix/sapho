---
id: SR-033
title: "Local request admission, observations and dashboard review"
type: SpecReview
analysis: code-review
scope: "Sapho request budgeting, prompt framing, generic backend observations and dashboard at 54bdaabb238b242295162bb3bd331105915797d6"
review_set: subset
---
# Local request admission, observations and dashboard review

## Summary

Self-review of the generic increment at 54bdaabb238b242295162bb3bd331105915797d6
against 06ae050e1eeac309008fbfe3248f4785c6e71ebd. Single-request admission facts
use the actual provider encoder and strict context/request caps. Explicit framing
retains complete typed content. Provider-independent observations preserve failures,
cancellation, available usage and response-retention failure without implying HTTP
send or valid answers. Dashboard freshness and assessment denominators are explicit.
These changes do not implement downstream whole-classification admission.

## Scope

Examined sapho-ollama/lib.rs and prompt.rs; sapho-campaign/meter.rs, execution.rs,
dashboard.rs and lib.rs; owning FR-048/050/052, API docs, CLI command wiring and
existing core bounded serialization/model validation as dependencies. Reviewed
workspace deny.toml and confirmed there are no dependency or CI changes. No
applicable AssuranceProfile was found. Repository CLAUDE.md and dev-tools Rust/
code-review conventions apply. This is self-review, not independent correctness
adjudication or a downstream application release review.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | The repository-wide strict matrix exits 1 for three existing cross-consumer/TUI criteria with no local binder; the generic increment cannot be described as a full strict matrix pass. | FR-051-AC-3; FR-051-AC-4; FR-052-AC-3 |

## Verification

Locked offline full workspace tests pass with all features (134 passed including
doctests) and without default features (132 passed including doctests). Each lane
has one explicitly ignored subprocess helper invoked by its parent capacity test.
Strict all-target Clippy passes both feature lanes. Formatting, diff, unsafe audit
script and documentation with warnings denied pass. Cargo-deny passes advisories,
bans, licenses and sources, with configured wildcard/unused-license warnings.
The initial audit could not cache missing target-specific crates under the sandbox;
authorized Cargo cache access completed acquisition. The audit then identified
three new workspace-owned AGPL crates missing from the repository's self-license
exceptions. Exact-package/version exceptions now apply only to those examined
0.1.0 crates; no external license allowlist or advisory check was relaxed.

Quire 0.36.1/engine 0.50.1 validates changed specs with installed catalog/schema
advisories. All nine FR-048 criteria, FR-050-AC-5 and FR-052-AC-5/6 have computed
trace binders. New criteria live once in their Acceptance Criteria section; no
second FR-048-AC-7 is retained. The broader strict matrix gap is recorded above.

Real loopback transport fixtures compare pure admission facts to actual captured
wire bytes and prompt components in both encodings. Explicit framing round-trips
Unicode, quotes, delimiter-shaped text, nested typed values, question metadata and
raw evidence. A one-byte context overflow and a one-byte wire overflow produce
no send. Invalid bounds are refused under the provider's shared validator. Legacy
prompt and JSON body encoding are byte-identical under the exercised contract.
The pure API performs no service probe, file access, capacity acquisition or HTTP.
Counting uses core's measured serializer; raw request hashing/encoding and body
allocation are bounded. Dispatch and measurement share the prompt/wire encoders
and context predicate, rather than separate numeric formulas.

Observation tests verify exact typed hashes/bodies, supplied usage, source-free
metadata, structured failure, dropped-future cancellation and response-retention
failure; the latter retains known usage without a fabricated response or retry.
Locks are released before awaiting delegates. Recording export remains separate.
Dashboard tests verify bounded read-only snapshots, exact selection, omitted old
optional fields decoding as unknown, future/stale time refusal, target versus
positive assessment denominator, and numerator/denominator evidence bounds.
No GPU inference, credentials or private campaign data were used.

## Verdict

CONDITIONAL for a full campaign release because the broad strict matrix gap
remains. No new code defect was found in the examined generic increment. A pinned
read-only dashboard/API battletest may be delivered with this scope and the actual
CLI receipt; it does not clear any downstream inference hold or imply completion.

## Dispositions

| Finding | Outcome | Reason |
|---|---|---|
| FND-001 | deferred | Cross-consumer adaptation and interactive terminal criteria require owning-lane evidence; this review neither invents local tags nor claims a whole application pass. |
