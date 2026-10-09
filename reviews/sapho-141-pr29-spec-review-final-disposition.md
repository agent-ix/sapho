---
id: SR-071
title: "Spec base review — SAPHO-141 local-run environment record"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@0f5cdc077dedcd97012eb6f11dd801c5b263569d; spec/modules/cli/functional/FR-055.md; docs/cli-guide.md; SAPHO-141"
review_set: subset
---

## Summary

Re-reviewed the corrective head against the Quoin hash/digest/pin/tracking-record checklist. The prior runbook record is removed, and the digest obligations in the full normative chain are replaced by response-model-name and raw-body language. A separate PR-touched CLI requirement still instructs operators to keep an environment record to prove locality, and the guide repeats it.

## Verdict

**FAIL** — one high tracking-record finding remains.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | FR-055 still instructs an operator to keep the run environment as a locality provenance record; the CLI guide repeats the instruction. | spec/modules/cli/functional/FR-055.md:48; docs/cli-guide.md:160 |

## Coverage

- Exact head `0f5cdc077dedcd97012eb6f11dd801c5b263569d`. Compared all 11 corrective files to `b155ab198cd836c86b170319ca8eb2311e6bb5a7`, plus the unchanged Rust implementation and scoped tests. No production source or test changed in the corrective commit.
- FR-005, FR-048, FR-053, IT-007, Ollama master scope, StR-012, US-012 and TC-055 no longer require a digest or exact answering-weights attribution. FR-052 and the runbook no longer call for an external operating-evidence record. The adapter still emits `digest: None` for generate/embed; evidence same-source exclusion still uses reported model-name equality.
- FR-055 behavior retains the instruction to keep the environment with the run to show it stayed local. The touched CLI guide repeats it. The environment is not a per-response proof, and this externally retained record falls under the Quoin checklist's high tracking-record prohibition.
- `quire validate --scope . 'spec/**/*.md' --summary` passed (197/197 grammar-clean). Computed Test Matrix: FR-005/036/048/052/053/054/055 criteria statically tagged. The corrected FR-053-AC-1 and IT-007-SC-01 now match the unchanged tests' response-name/no-digest behavior.

## Dispositions

| FND | Outcome | SHA/reason |
| --- | --- | --- |
| FND-001 | fixed | 0b25ea6579710d231237fb9b95e0edd426543be8 — FR-055 now says the operator chooses the intended server before a live call; the CLI guide gives the same action and no longer asks to keep an environment record. |

## Re-review coverage

- Rechecked all PR-touched spec and docs paths for positive digest, pin, manifest, receipt or tracking-record obligations. Remaining mentions deny answering-weight attestation, describe raw exchanges or explain routine recordings; none establishes a new proof record.
- Earlier FR-005/048/053, IT-007, Ollama master scope, StR-012, US-012 and TC-055 digest residuals remain corrected. FR-052 and the provenance runbook still avoid a new operating-evidence record.
- No Rust source or tests changed since `0f5cdc077dedcd97012eb6f11dd801c5b263569d`. Generate/embed still emit `digest: None`; evidence same-source exclusion remains name-only. Spec validation reports 197/197 grammar-clean documents.
