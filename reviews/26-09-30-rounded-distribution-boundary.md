---
id: SR-006
title: "Sapho approximate probability distribution boundary"
type: SpecReview
analysis: code-review
scope: "sapho@75af06b36e34d9bf6dd3de61a40edcd91aecb843; CLAUDE.md, AGENTS.md, Cargo.toml, crates/sapho-jev/Cargo.toml, crates/sapho-core/src/{model,value,ports,tests}.rs, crates/sapho-jev/src/{lib,tests}.rs, crates/sapho-runtime/src/{engine,operators,trace}.rs, crates/sapho-recording/src/lib.rs, spec/modules/core/functional/FR-005-validate-model-answers.md, spec/modules/logic/functional/FR-020-project-model-probability-mass.md, spec/modules/recording/functional/{FR-027-record-successful-backend-exchanges,FR-028-replay-exact-recorded-requests}.md; consumer quire-semantic@4f08e7a075fea08046bd945a0583d2a861072a45 graphs/ears.toml and crates/quire-semantic-{core,eval}/src; authoritative typesafe-sdk-answers/client/models/questions 0.6.2 source and SDK response fixture test; local private smoke manifest/README, two failed traces, successful recording and saved live/replayed reports; provider OpenAPI and Choice/Score/Confidence docs inspected 2026-09-30"
review_set: subset
---
# SR-006: Sapho approximate probability distribution boundary

## Summary

Saved EARS development runs expose a reusable mismatch between Sapho's exact
complete-distribution contract and Jev's approximately normalized output.
This assessment uses existing local evidence; no engine or consumer code,
model call, test run, dependency installation or external ticket was performed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Complete, individually valid Jev distributions at total mass 0.99 abort the graph; no explicit approximate-distribution policy can distinguish these from materially malformed answers. Two of three inspected development passages abort at roles-ask. | crates/sapho-core/src/model.rs:303; crates/sapho-core/src/ports.rs:93; spec/modules/core/functional/FR-005-validate-model-answers.md:29; crates/sapho-jev/src/lib.rs:87 |

## Verdict

FAIL for the observed live EARS integration: FND-001 remains open. Implementation
matches FR-005 as currently written; the integration exposes an overly strict
requirement, rather than an implementation violating that requirement.
This finding does not establish EARS classification accuracy or a population
failure rate.

## Evidence and limits

The local smoke manifest identifies Sapho 75af06b, quire-semantic 4f08e7a,
SDK 0.6.2, requested jev-latest and returned jev-1.13.0. The current EARS graph
digest matches the manifest. Private passages and responses remain in their
authoritative data directory; none are copied into this repository.

| Saved case | Independently inspected result |
|------------|--------------------------------|
| EAP-004 | Structure completes. roles-11 contains all nine declared labels, each probability in [0,1], total mass 0.99; roles-ask fails InvalidAnswer. |
| EAP-001 | Structure completes. roles-0, roles-21 and roles-50 have all declared labels, total mass 0.99; roles-ask fails InvalidAnswer. |
| EAP-017 | Saved recording has four exchanges and 64 answers, all mass checks within the existing tolerance. Saved live and replayed report objects and file SHA-256 digests match. Replay was not rerun in this assessment. |

EAP-004's 22 observed distributions lie on a 0.01 grid. That is evidence
consistent with rounding, not proof of the provider's internal rounding algorithm
or a guaranteed precision contract. Binary floating-point roundoff cannot
explain a deficit of 0.01.

The provider's [OpenAPI contract](https://api.typesafe.ai/openapi.json) describes
both Choice and Score masses as summing to approximately one, without a numeric
error limit or rounding mode. The narrative [Choice docs](https://docs.typesafe.ai/primitives/choice)
and [Score docs](https://docs.typesafe.ai/primitives/score) describe ideal unit
mass. The local SDK stores f64 probabilities unchanged; its captured-response
test checks unit mass for its particular fixture, not every provider response.
The numeric allowance must therefore be an explicit integration policy, not a
claim that an exact provider precision guarantee has been verified.

## Existing ownership and evidence preservation

- JevBackend translates the authoritative SDK answer into raw typed core fields
  without changing probability values (crates/sapho-jev/src/lib.rs:87).
- Runtime retains the raw ModelResponse before validating it
  (crates/sapho-runtime/src/engine.rs:318). The failed traces inspected contain
  the original masses. This preserves typed answer evidence, not original HTTP
  bytes, property ordering or discarded SDK fields.
- Answers::validate and probability projection reapply the same invariant
  (crates/sapho-core/src/model.rs:167,197). An adapter-only exception would fail
  again unless it rewrote evidence or changed the shared answer representation.
- Recording validates before retention, export and load. Under the current
  contract the failed exchanges remain in failure traces, not successful
  recordings (crates/sapho-recording/src/lib.rs:52,137,169).
- Distribution coverage, approximate mass and model confidence are distinct.
  In EAP-004 roles-11, reported confidence is 0.51 while the selected outcome's
  raw probability is 0.56. Neither should replace the other.

## Concrete reusable issue

Title: Support bounded approximate complete distributions with unchanged raw evidence.

Owner: Sapho core answer boundary, with Jev host binding policy and recording/replay
integration. EARS supplies the consumer scenario; no EARS-specific repair belongs
in the engine or the consumer.

Recommended first implementation:

1. Introduce a typed distribution policy on the host backend binding and copy it
   into the core ModelRequest. Keep strict validation available for other
   backends. Persist the policy with recordings so it participates in exact
   replay request matching. It is a local interpretation setting, not an SDK
   request field or a provider promise.
2. For an explicitly approximate complete distribution, require exact coverage of
   declared labels, finite unit-range entries and positive total mass. Accept
   only a configured small absolute mass error. Start the Jev integration at
   0.01 plus numerical roundoff tolerance; this is an empirical acceptance
   policy, not a verified provider guarantee. Larger discrepancies refuse.
3. Preserve ModelResponse unchanged. The core response boundary derives a
   separate canonical Answers distribution by p_i / total_mass only under the
   approximate policy. Retain the raw mass, policy and applied scale as
   auditable interpretation evidence. Probability graph nodes then project
   canonical mass, retaining the existing Probability unit-range invariant.
4. Keep partial and unavailable distributions distinct. Never add missing
   labels or normalize a partial distribution. Keep reported selection,
   confidence, score and actual model identity unchanged; a reported score is
   not recomputed from rounded probabilities.
5. Use this same core interpretation function for runtime and recording
   validation. Replay returns the unchanged recorded raw response, then
   reproduces the canonical answers under its recorded policy.
6. Add structured diagnostic context to mass refusal: question ID, coverage,
   supplied mass and permitted error. This makes future boundary failures
   attributable without requiring consumers to parse error message text.

This deliberately changes FR-005's blanket no-renormalization requirement.
FR-020 must define projection over canonical answers when the policy applies;
FR-027/028 must retain raw response plus interpretation policy and reproduce the
same derived values. Specify and review these changes before implementation.

| Approach | Assessment |
|----------|------------|
| Increase the global tolerance while keeping raw Answers | Insufficient: full/subset projections can exceed one for mass 1.01; floating-point tolerance and provider approximation remain conflated. |
| Normalize inside JevBackend and overwrite ModelResponse | Loses original probability evidence and obscures the adjustment in traces/recordings. |
| Explicit core interpretation, raw response retained | Recommended: preserves evidence, makes the allowance visible and keeps graph/replay semantics coherent for any backend. |

Normalizing this shape makes it usable by the engine; it does not establish
calibration, correct semantic labels, or a probability guarantee for combined
fuzzy logic.

## Proposed acceptance criteria

- Under the approximate policy, original synthetic complete Choice and Score
  distributions with masses 0.99 and 1.01 are accepted; canonical projections
  are in [0,1], all-label projection is one within roundoff, and raw entries
  remain unchanged in trace and recording.
- The same nonunit distributions refuse under the strict policy. Gross error,
  zero mass, unknown labels, wrong answer type and non-finite/out-of-range
  values refuse regardless of the approximation policy.
- A missing-label distribution remains Partial; existing known-mass projection
  and missing-mass refusal are unchanged. Partial excess mass still refuses.
- Reported selected label, confidence, expected score and actual identity survive
  unchanged, including cases where confidence differs from selected probability.
- Save/load/exact replay reproduces the raw response, interpretation metadata,
  canonical answers and downstream graph output without a live delegate.
  A changed interpretation policy causes a replay miss.
- Failed interpretation retains the raw typed response and useful question/mass
  diagnostic context. No private smoke artifact is used as a committed fixture.

## Verification performed

Existing source and saved JSON were read, numeric summaries computed locally,
and saved report equality/digests checked. No Rust test, lint, full gate or
live inference was run, because this is an assessment with no code change.
This review artifact alone is validated with the installed Quire command.

## Dispositions

FND-001 remains open pending the specified reusable boundary change. No source
fix, downstream repair, compatibility layer or external ticket/comment was
introduced.
