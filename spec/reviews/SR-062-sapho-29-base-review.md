---
id: SR-062
title: "SAPHO-29 repeated ask specification review"
type: SpecReview
analysis: base
scope: "FR-074, FR-075, FR-076 and IT-014 against current origin/main"
review_set: subset
---
# SR-062: SAPHO-29 base review

## Summary

Reviewed the public ModelRequest, Ask compile ports, runtime scheduling/RunLimits and ReplayBackend exact-key implementation on `origin/main` `e77b81eddd90b606561f2ed606cde74f133ea937`. This is a specification review; no implementation or live-data evidence is claimed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Mapping an index into Ask state changes the provider request and cannot meet identical-byte capture. | FR-074, FR-075 |
| FND-002 | high | Current ReplayBackend rejects different answers for duplicate exact requests. | FR-074 |
| FND-003 | medium | Current Ask has singular outputs and one model evidence slot. | FR-075, FR-076 |
| FND-004 | medium | A repeat count could cross RunLimits after sending only some calls. | FR-076, IT-014 |
| FND-005 | medium | The held-out gate uses primary plus two additional whole-graph runs, while an Ask samples one node. | FR-076 |
| FND-006 | high | Concurrent failures could change the returned error and truncate dispatched evidence according to completion timing. | FR-076, IT-014 |
| FND-007 | medium | Public List identity uses string ItemIds and paired answer/model lists need an exact join key and source rule. | FR-075, IT-014 |
| FND-008 | high | Per-Ask full-width waves and separate request prechecks could exceed run-wide concurrency or request ceilings when ready Asks overlap. | FR-076, IT-014 |

## Dispositions

FND-001: FR-075 puts `samples` on Ask; FR-074 keeps the index only in core identity and excludes it from provider projection. FND-002: FR-074 makes each repeated index part of the exact request key while retaining conflict refusal for duplicate same-index answers. FND-003: FR-075 declares separate repeated ports; FR-076 requires per-sample evidence and unchanged k=1 shape. FND-004: FR-076 requires pre-dispatch call-budget check and IT-014 covers the boundary. FND-005: FR-076 shares zero-based terminology but distinguishes metric populations and has no SAPHO-33 implementation dependency.

FND-006: FR-076 now uses fixed concurrency waves, waits for the whole dispatched wave, selects the lowest failing index and retains all wave outcomes; IT-014-SC-03/05 check k=4 replay, dual failure, timeout and un-dispatched calls. FND-007: FR-075 defines canonical decimal ItemIds, paired ordering and inherited SourceRefs; IT-014-SC-07 checks exact typed JSON round-trip and downstream join.

FND-008: FR-076 now defines one shared Engine::run admission ledger and permit pool for sibling Asks and Map children. Atomic whole-Ask reservations cap all admitted work, actual dispatch consumes reservations, unused slots are released, and each infer holds one shared permit. IT-014-SC-08 checks two parallel repeated Asks at exact and insufficient request ceilings, adversarial response order, active high-water mark and mapped-child accounting.

## Verdict

Ready for planner review of the public contract and executable acceptance cases. `quire validate --scope . 'spec/**/*.md'` exited 0; `quire matrix --scope .` exited 0 and showed all fourteen new ACs untagged; `git diff --check` exited 0. IT-014 is planned evidence, not executed implementation evidence. `filament open` failed at the OS URL opener with error -10661, so visual IDE opening could not be checked. Dataset serde/validate and the EARS export boundary remain outside this change.
