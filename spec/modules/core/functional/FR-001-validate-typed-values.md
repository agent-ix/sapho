---
id: FR-001
title: "Validate typed values"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-004"
    type: "references"
  - target: "ix://agent-ix/sapho/NFR-001"
    type: "references"
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
---
# FR-001: Validate typed values

## Description

The Sapho value boundary SHALL reject values that do not satisfy their declared type.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Value types are Boolean, finite Number, Text, Probability, Degree, Optional(inner), List(inner), Record(fields), Questions and Answers. Records have exactly their declared fields. Lists contain Datum items; each item has a non-empty ItemId, a Value and source references. Item IDs are unique within each list. Optional absence is distinct from false, zero and an empty list. Constructors and deserialization are both validated; recursion is capped at 32 levels. Probability and Degree each accept finite values in [0,1] and remain distinct types. Identity types distinguish node, item, backend, primitive and source identifiers. Names are non-empty strings; node and port names use ASCII letters, digits, underscores or hyphens. Numeric boundary conversions are fallible.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-001-AC-1 | A declared record and nested list round-trip with their exact values and item IDs. | Test (TC-001) |
| FR-001-AC-2 | NaN, infinity, out-of-range probabilities/degrees and invalid source ranges are rejected as InvalidValue; repeated item IDs use DuplicateId, and incompatible record schemas use TypeMismatch. | Test (TC-001) |
| FR-001-AC-3 | Optional absence is accepted only for Optional ports; a 33-level value is rejected without a panic. | Test (TC-001) |

## Dependencies

No functional prerequisites beyond the module stakeholder need.
