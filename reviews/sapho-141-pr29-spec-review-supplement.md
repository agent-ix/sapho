---
id: SR-069
title: "Supplemental spec base review — SAPHO-141 PR 29 operating record"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@b155ab198cd836c86b170319ca8eb2311e6bb5a7; docs/ollama-provenance.md; FR-052; SAPHO-141"
review_set: subset
---

## Summary

Reviewed the revised Ollama provenance procedure against the Quoin hash/digest/pin/tracking-record gate. The new procedure creates a detailed run tracking record despite the PR's goal of removing unsupported provenance claims. An independent exact-head review already recorded residual digest obligations in FR-005/048/053, TC-055, master scope, US-012, StR-012 and IT-007; this supplemental review records a separate new instruction.

## Verdict

**FAIL** — the new operating record is a high Quoin checklist finding.

## Findings

| ID | Severity | Summary | Refs |
| --- | --- | --- | --- |
| FND-001 | high | New Ollama provenance procedure instructs operators to record and retain an operator/PID/version/times/write-action tracking record. | docs/ollama-provenance.md:60 |

## Coverage

- Exact head `b155ab198cd836c86b170319ca8eb2311e6bb5a7`. The procedure at lines 44-64 says to save checks with the run, then record operator, server PID and port, Ollama version, model name, store permissions, start/end times, startup compute line and every model-write action, retaining that evidence with recordings.
- The Quoin spec-review checklist flags an instruction creating a tracking record as high unless a canonical identity digest binds a proof to exact content. This record is not such a proof; FR-052 correctly states that a model name and availability check cannot attest answering weights.
- The controlled dedicated-instance procedure can be documented without a new required tracking record. This finding is about the new record obligation, not about operating a dedicated instance.

## Dispositions

| FND | Outcome | SHA/reason |
| --- | --- | --- |
| FND-001 | fixed | 0f5cdc077dedcd97012eb6f11dd801c5b263569d — `docs/ollama-provenance.md` no longer asks to save checks or record operator/PID/version/times/write actions; `docs/cli-guide.md` no longer asks for an isolation verification record. |
