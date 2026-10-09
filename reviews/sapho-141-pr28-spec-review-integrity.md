---
id: SR-064
title: "Integrity review of Ollama digest identity across modules"
type: SpecReview
analysis: integrity
scope: "agent-ix/sapho@5be1e7061fe266e9951faa0dc551c4898c0460b9; spec/modules/ollama/functional/FR-052.md, spec/modules/evidence/functional/FR-036.md, spec/modules/cli/functional/FR-055.md"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-052"
    type: references
---
# SR-064: Digest identity integrity review

## Summary

Ticket: SAPHO-141. Checked the new FR-052 observation contract against evidence and CLI requirements that consume the same ModelIdentity.

## Verdict

FAIL until the dependent requirements consistently distinguish current binding observation from historical answer provenance. A current lookup cannot validate or invalidate an archived model-made label by itself.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | Cross-module contract still treats observed digest as if it attested every answering model | spec/modules/evidence/functional/FR-036.md:35 |
| FND-002 | medium | CLI requirement describes digest as server-reported, conflicting with FR-052 derivation | spec/modules/cli/functional/FR-055.md:55 |

## Coverage

- FR-052 description, behavior and AC-7 (examined): pre-request observation and external retag limitation.
- FR-036 behavior and AC-5 (examined): name/digest self-source exclusions, including historical label declaration.
- FR-055 behavior and AC-3 (examined): provenance carried in recordings.

## Dispositions

| FND | outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
| FND-002 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
