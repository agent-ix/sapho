---
id: SR-065
title: "SAPHO-22 calibration specification review"
type: SpecReview
analysis: base
scope: "FR-081 through FR-084 and IT-017 against current origin/main and draft SAPHO-21/25 specifications"
review_set: subset
---
# SR-065: SAPHO-22 base review

## Summary

Reviewed current core Probability/Degree ValueType, graph compiler ports, pure evidence Dataset/measure, CLI replay and exact source identity on `origin/main` `e77b81eddd90b606561f2ed606cde74f133ea937`. Reviewed draft FR-065 ECE in SAPHO-21 PR #34 and draft FR-063 semantic graph identity in SAPHO-25 PR #33 as implementation dependencies. This is specification evidence only; neither draft feature is claimed implemented or merged.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Treating a calibrated number as raw Probability would erase the distinction required by core semantics. | FR-081, FR-082 |
| FND-002 | high | Binding a map to the calibrated graph's own digest would be circular because embedding the map changes that graph. | FR-081, FR-082 |
| FND-003 | high | Fitting on held-out labels or silently dropping failed/self-source development rows could leak or distort calibration. | FR-083, FR-084 |
| FND-004 | medium | Isotonic fit/interpolation and minimum data rule are unspecified in the ticket and could yield nondeterministic bytes. | FR-083, IT-017 |
| FND-005 | medium | New ECE math or Quoin dependency in calibration would diverge from SAPHO-21 parity and evidence boundaries. | FR-083, IT-017 |
| FND-006 | medium | A map fit artifact needs dataset content and graph identity while preserving existing GraphArtifact.source and Dataset serde. | FR-081, FR-083, FR-084 |
| FND-007 | medium | Map changes downstream of Ask should change graph identity without invalidating exact recording keys. | FR-082, IT-017 |
| FND-008 | high | Graph and Dataset digests do not identify the model selected outside GraphSpec; an old map could be used after the provider reports a different actual model. | FR-081, FR-083, FR-084, IT-017 |

## Dispositions

FND-001: new CalibratedProbability Value/ValueType and explicit `calibrated_as_probability` conversion. FND-002: map stores raw fitting graph digest; calibrated graph digest includes map literal separately. FND-003: fit accepts only development and requires fully scored eligible rows with independent provenance; held-out measure remains permitted. FND-004: integer-comparison weighted PAVA, minimum 20 rows/classes/distinct p, fixed knots/interpolation and sorted canonical digests. FND-005: shared FR-065 ECE implementation and retained Quoin parity test are implementation prerequisites; no Quoin runtime dependency. FND-006: map embeds dataset ID and selected development-content digest, while existing source fields and Dataset shape are stable. FND-007: IT-017 compares graph digests and exact replay for two downstream maps.

FND-008: fit now requires one contributing Ask/BackendId and uniform reported actual model, stores both plus a scored-observation digest in the map ID, and refuses ambiguous attribution. The stock CLI checks each calibrated output against actual trace responses in run/replay/measure, refuses changed or unavailable identity with ModelMismatch, and reports a successful name-only match without claiming weights equivalence. Development remeasurement of the original case set also compares the fit-observation digest; IT-017-SC-07 exercises changed model, same-name changed predictions and ambiguous attribution on unchanged graph/Dataset bytes. The graph `calibrate` operation remains pure.

## Verdict

Ready for planner review of the explicit fit/apply/score contract. `quire validate --scope . 'spec/**/*.md'` exited 0; `quire matrix --scope .` exited 0 and showed all twenty new ACs untagged; `git diff --check` exited 0. `filament open` could not dispatch the IDE URL through the OS opener (error -10661). IT-017 is planned, not executed implementation evidence. FR-063 and FR-065 must be implemented before the corresponding SAPHO-22 identity/ECE acceptance cases can pass. Dataset serde/validate and the EARS Sapho-Datasets config path remain untouched.
