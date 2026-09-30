---
id: FR-002
title: "Preserve source attribution"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
---
# FR-002: Preserve source attribution

## Description

The Sapho execution boundary SHALL retain caller-supplied source references on derived data.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

SourceRef carries an opaque SourceId and optional half-open byte range start/end; either both offsets are present or neither is present, with start <= end. The library never opens the source identifier. Datum owns item identity and source references separately from its value. Outputs inherit the stable deduplicated union of input references, plus references explicitly supplied by code. Collection items retain their own references; map execution includes the focal item and captured-context references. No text-based deduplication is performed.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-002-AC-1 | Two items with equal text and different IDs/spans remain separate. | Test (TC-002) |
| FR-002-AC-2 | A mapped judgment retains its focal span and captured context attribution. | Test (TC-002) |
| FR-002-AC-3 | A code-supplied additional source reference appears alongside inherited references. | Test (TC-002) |

## Dependencies

- [FR-001](FR-001-validate-typed-values.md)
