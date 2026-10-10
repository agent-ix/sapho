---
id: SR-059
title: "Base review of SAPHO-34 slice and drift specification"
type: SpecReview
analysis: base
scope: "SAPHO-34; spec/spec.md; FR-067..068; IT-011; evidence module index"
review_set: base
---
# SR-059: Base review of SAPHO-34 slice and drift specification

## Summary

The requirements define a deterministic labelled slice/window report and a separate label-free comparison of retained recording confidence. The review checked the five ticket acceptance checks, denominator and delta direction, missing-value identity, small-slice behavior, window provenance, recording deduplication, pure evidence boundaries and synthetic fixture arithmetic. This is specification evidence only.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | Dataset and Recording have no timestamps. Window membership is explicit caller input; recording names label supplied sets, and no report claims event-time derivation. | FR-067; FR-068 |
| FND-002 | low | A Recording retains successful exchanges and may deduplicate identical requests. Confidence distance therefore describes retained exchanges, not invocation frequency or population drift prevalence. | FR-068 |
| FND-003 | low | Label-free mean/share movement cannot establish calibration drift or accuracy. The report does not synthesize labels, ECE, risk, alerts or recalibration. | FR-068 |
| FND-004 | low | The 12 new criteria are untagged in the computed matrix until implementation tests bind them. | FR-067; FR-068 |

## Review Evidence

The four-case labelled fixture has agreements `1.0`, `0.5` and `0.75` for A, B and overall, with signed deltas `+0.25` and `-0.25`. The minimum count is based on selected cases, so threshold 3 flags both two-case slices without suppressing their metrics. The missing-key variant is structurally distinct from literal Text `missing`. Explicit window membership also yields a slice-by-window report, including absent deltas where a slice has no case in one window. Recording confidences `[0.6,0.8]` and `[0.7,0.9]` yield means `0.7` and `0.8`; inclusive threshold `0.8` retains one of two in each set. Stable case-ID and numeric-confidence accumulation order prevents insertion order from changing serialized floating results. FR-067 reuses SAPHO-21 ECE and risk definitions when available and explicitly marks them unavailable otherwise. Inputs are existing typed fields and caller-provided window membership; Dataset serde/validate and the EARS production boundary remain intact.

`quire validate --scope . 'spec/**/*.md'` exited 0 with installed catalog diagnostics only; `git diff --check` exited 0; `quire matrix --scope . --format tsv` reported 12 scoped criteria `untagged`. No Rust, private EARS data or live 1,000-passage run is part of this specification stage.
