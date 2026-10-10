---
id: SR-060
title: "Base review of SAPHO-33 promotion gate specification"
type: SpecReview
analysis: base
scope: "SAPHO-33; spec/spec.md; FR-069..071; IT-012; evidence and CLI module indexes"
review_set: base
---
# SR-060: Base review of SAPHO-33 promotion gate specification

## Summary

The scoped requirements define a gate snapshot registered before held-out measurement, bound to raw and semantic gate identities plus Dataset content and semantic graph identity. Six mandatory bars per output are evaluated on complete selected cases. This review checked all six ticket acceptance checks, baseline and class arithmetic, absent-value refusal, identity drift, repeated predictions, exit-code precedence and crate boundaries. It is a specification review, not implementation or real promotion acceptance.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | A local exclusive receipt proves the command wrote the gate snapshot before its own evaluation, but does not provide trusted external time attestation. The gate report identifies exact bytes and semantic content; an edited file cannot reuse the receipt. | FR-069 |
| FND-002 | low | A Boolean-only output has no ECE; because all six bars are mandatory, it remains NO-GO until a typed Probability output supplies calibration evidence. There is no implicit Boolean-to-confidence pairing. | FR-070 |
| FND-003 | low | An unavailable Quoin-parity-backed SAPHO-21 ECE stays `not_computed`; the gate may not duplicate the estimator. Quoin parity PR #687 is open and its merged SHA/output receipt remains an implementation acceptance dependency. | FR-070; IT-012 |
| FND-004 | low | Dataset and graph content identities are required in addition to caller-owned Dataset ID; existing `GraphArtifact.source` and Dataset interchange remain unchanged. | FR-069; FR-071 |
| FND-005 | low | The 16 new acceptance criteria remain untagged until observable implementation tests bind them. | FR-069 through FR-071 |

## Review Evidence

The tie fixture has `3/4` agreement and a `3/4` constant baseline, so margin zero fails strict `>0`. The eight-case fixture has agreement `7/8`, baseline `1/2`, margin `3/8`, true recall `4/4`, false recall `3/4`, and ECE `1/4` under SAPHO-21's predicted-label deciles; two identical repetitions have disagreement zero. A case missing from any required repetition is incomplete evidence and retains exit 2, while a complete bar failure uses exit 1 in the existing CLI 0/1/2 scheme. An edited gate cannot relabel a result because its byte digest differs from the pre-measurement receipt. A development split has no promotable outcome. No metric couples to a backend model name.

`quire validate --scope . 'spec/**/*.md'` exited 0 with installed catalog diagnostics only; `git diff --check` exited 0; `quire matrix --scope . --format tsv` reported 16 scoped criteria `untagged`. No Rust test, Quoin merge receipt, EARS private Dataset or live 1,000-passage measurement is claimed at this stage.
