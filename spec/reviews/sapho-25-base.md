---
id: SR-057
title: "Base review of SAPHO-25 measured roster specification"
type: SpecReview
analysis: base
scope: "SAPHO-25; spec/spec.md; FR-061..064; IT-009; affected module indexes"
review_set: base
---
# SR-057: Base review of SAPHO-25 measured roster specification

## Summary

The scoped requirements define a roster assembled from observed backend calls, attributed case outcomes, live-only timing and declared provider metadata. The review checked the six ticket acceptance checks, crate boundaries, identifier uniqueness, links, failure and absence semantics, split isolation, graph-literal typing and resource limits. The contracts are ready for external specification review; no implementation behavior is claimed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | A graph author can copy or edit a roster Record literal, and the compiler cannot certify that the copied fields came from measurement. The roster writer accepts no caller metric values and the artifact states its provenance; measured-origin attestation would require a separate trust contract if desired. | FR-063; FR-064 |
| FND-002 | low | ECE stays explicitly not-computed until SAPHO-21 supplies the shared metric; the roster does not introduce a second estimator. | FR-062-AC-5 |
| FND-003 | low | The 20 new criteria are untagged in the computed matrix because implementation tests have not begun; bind them to observable tests before claiming feature completion. | FR-061 through FR-064 |

## Review Evidence

FR-061 separates nondeterministic live timing from deterministic Trace and recording while observing any ModelBackend. FR-062 counts actual and unknown model identities, states usage and latency denominators, refuses mixed live/replay timing, excludes unattributable outputs, and keeps Dataset serde and validation stable. FR-063 specifies a typed Record projection and a canonical graph identity digest that binds measured provenance to exact graph content; this is the narrow identity use allowed by the review checklist, not a file inventory or process pin. FR-064 selects one Dataset split, writes exclusively within existing bounds, gives ordinary `measure` per-kind views, and omits replay latency. IT-009 uses only synthetic backends and an injected clock, with explicit success criteria for all six ticket checks.

`quire validate --scope . "spec/**/*.md"` and `git diff --check` exited 0. `quire matrix --scope . --format tsv` shows 20 scoped criteria as `untagged`, pending implementation tests. No Rust or private EARS dataset tests were run for this specification change.
