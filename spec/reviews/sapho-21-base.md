---
id: SR-058
title: "Base review of SAPHO-21 calibration specification"
type: SpecReview
analysis: base
scope: "SAPHO-21; spec/spec.md; FR-065..066; IT-010; evidence module index"
review_set: base
---
# SR-058: Base review of SAPHO-21 calibration specification

## Summary

The scoped requirements port Quoin's confidence-based ECE and inclusive risk-coverage count rule into pure Rust evidence measurement. This review checked all six ticket acceptance checks, hand arithmetic, empty and unsupported results, threshold validation, Dataset compatibility, and the `sapho-evidence` dependency boundary. The specification defines acceptance behavior; it does not claim implementation.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | The ticket left confidence semantics open. This specification selects predicted-label confidence `max(p,1-p)` to match Quoin's `jev.ece-v1` convention; P(true) reliability remains a distinct later question. | FR-065; IT-010 |
| FND-002 | low | Pairing a separate Boolean decision output with a confidence output would need explicit case/output lineage and is outside this Probability-output slice. | FR-066 |
| FND-003 | low | ECE-based candidate ranking and calibration-map fitting are separate tickets; this change preserves existing `tune` ranking and Dataset interchange. | FR-065; FR-066 |
| FND-004 | low | The ten new acceptance criteria have no implementation trace tags yet. Bind them to observable tests before claiming feature completion. | FR-065; FR-066 |
| FND-005 | low | Quoin's reference function is private to its test crate, so Sapho cannot import it. IT-010 now requires a committed test in Quoin's existing `grading_math` crate, a focused passing command, and exact SHA/output links in the implementation PR, alongside Sapho's independent fixture test. | FR-065-AC-1; IT-010-SC-01 |

## Review Evidence

The ten-case fixture gives decile contributions `0.085`, `0.015`, `0.15`, `0.07` and `0.01` for buckets 9 through 5, totaling `0.33`. Bucket 9 contains confidence values `0.95`, `0.9`, `1.0` with two correct, so mean confidence is `0.95` and observed accuracy is `2/3`. Hand-counted risk rows appear in IT-010 and test the `>=` boundary independently. Quoin's reference implementation uses `min(floor(10c),9)`, mean confidence per occupied bucket and a count-weighted absolute gap; its coverage curve includes confidence equal to a threshold. Empty ECE is absent, Degree/Number stay unsupported, and thresholds are bounded and validated. The original `measure` signature, Dataset JSON/serde/validation and only-core workspace dependency are explicit compatibility checks.

At initial PR head `03cb0390328af7f9ed60c634c1ace87e21ec9a1d`, `quire validate --scope . 'spec/**/*.md'` exited 0 with installed catalog diagnostics only, `git diff --check` exited 0, and `quire matrix --scope . --format tsv` reported ten new criteria `untagged`. The exact revised-head validation is recorded in the PR update. No Rust or EARS private dataset test is claimed for this spec stage; Quoin parity is an implementation acceptance gate with durable cross-repository evidence.
