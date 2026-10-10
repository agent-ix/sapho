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

The scoped requirements define a roster assembled from observed backend calls, actual-model-attributed case outcomes, live-only timing and bounded provider descriptors. The review checked the six ticket acceptance checks, crate boundaries, identifier uniqueness, links, failure and absence semantics, split isolation, graph-literal typing, graph identity and resource limits. Planner findings on attribution, descriptor leakage and identity compatibility have been incorporated; no implementation behavior is claimed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | A graph author can copy or edit a roster Record literal, and the compiler cannot certify that the copied fields came from measurement. The roster writer accepts no caller metric values and the artifact states its provenance; measured-origin attestation would require a separate trust contract if desired. | FR-063; FR-064 |
| FND-002 | low | ECE stays explicitly not-computed until SAPHO-21 supplies the shared metric; the roster does not introduce a second estimator. | FR-062-AC-5 |
| FND-003 | low | The 23 new criteria are untagged in the computed matrix because implementation tests have not begun; bind them to observable tests before claiming feature completion. | FR-061 through FR-064 |
| FND-004 | low | A binding may return multiple actual model IDs. Each case/output is scored only under the entry matching its single contributing response's actual model; unknown and ambiguous cases have no scored denominator. A synthetic two-ID plus response-less fixture checks per-entry counts and denominators. | FR-062-AC-7; IT-009-SC-01/02 |
| FND-005 | low | Provider metadata formerly accepted an arbitrary host descriptor. The public boundary now permits only two bounded nonsecret identifiers and refuses unknown, endpoint or credential fields; roster and literal use a strict projection whitelist. | FR-061-AC-5; FR-062; FR-063-AC-4; FR-064-AC-4; IT-009-SC-05 |
| FND-006 | low | The new roster semantic graph digest is versioned and canonicalizes typed GraphSpec data, named map order and finite numeric values while preserving sequence order. Existing path/raw-byte `GraphArtifact.source` and ordinary `measure` source fields retain their current behavior. | FR-063-AC-2/5; FR-064; IT-009-SC-04 |

## Review Evidence

FR-061 separates nondeterministic live timing from deterministic Trace and recording while observing any ModelBackend through a validated metadata boundary. FR-062 counts actual and unknown model identities, partitions scored case IDs by the contributing actual response, states usage and latency denominators, refuses mixed live/replay timing, excludes unattributable outputs, and keeps Dataset serde and validation stable. FR-063 specifies a typed Record projection and a versioned semantic graph digest that binds measured provenance to exact graph content while retaining the existing source identity; this is the narrow identity use allowed by the review checklist, not a file inventory or process pin. FR-064 selects one Dataset split, writes exclusively within existing bounds, gives ordinary `measure` per-kind views, and omits replay latency. IT-009 uses only synthetic backends and an injected clock, with explicit success criteria for all six ticket checks.

`quire validate --scope . "spec/**/*.md"` and `git diff --check` exited 0. `quire matrix --scope . --format tsv` shows 23 scoped criteria as `untagged`, pending implementation tests. No Rust or private EARS dataset tests were run for this specification change.
