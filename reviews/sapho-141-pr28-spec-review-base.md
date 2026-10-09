---
id: SR-063
title: "Base spec review of Ollama digest contract"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@5be1e7061fe266e9951faa0dc551c4898c0460b9; spec/modules/ollama/functional/FR-052.md, spec/modules/evidence/functional/FR-036.md, spec/modules/cli/functional/FR-055.md, docs/ollama-provenance.md"
review_set: subset
relationships:
  - target: "ix://agent-ix/sapho/FR-052"
    type: references
---
# SR-063: Digest contract base review

## Summary

Ticket: SAPHO-141. FR-052 now correctly calls the digest a pre-request observation and states the server-side write-control condition. Related existing requirements still describe the value as answering-weight evidence without that condition.

## Verdict

FAIL due to cross-spec contradictions in the identity contract. The PR's primary FR-052 wording matches the official Ollama API response shape; the broader contract has to use the same qualified meaning.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | FR-036 promises unconditional self-source exclusion using a digest that can misidentify answering weights | spec/modules/evidence/functional/FR-036.md:35 |
| FND-002 | medium | FR-055 says the server reports a weights digest although Sapho derives it from show modelfile | spec/modules/cli/functional/FR-055.md:55 |

## Coverage

- FR-052 description, behavior and AC-1 through AC-7 (examined): digest meaning, refusal limits, race and documentation criterion.
- FR-036 behavior and AC-5 (examined): historical model label provenance and self-source comparison.
- FR-055 behavior and AC-3 (examined): recording identity source and derivation.
- The runbook (examined) explicitly rejects second-show proof and requires writer isolation for actual runs. It does not claim that this PR itself establishes an isolated pilot instance.

## Dispositions

| FND | outcome | sha/reason |
| --- | --- | --- |
| FND-001 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
| FND-002 | fixed | 16f3404de31231b5122cd89ecd89fc7f285645bd |
