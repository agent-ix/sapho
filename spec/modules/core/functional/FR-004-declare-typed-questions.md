---
id: FR-004
title: "Declare typed questions"
type: FR
relationships:
  - target: "ix://agent-ix/sapho/NFR-003"
    type: "references"
  - target: "ix://agent-ix/sapho/US-001"
    type: implements
  - target: "ix://agent-ix/sapho/StR-001"
    type: traces_to
  - target: "ix://agent-ix/sapho/FR-001"
    type: depends_on
---
# FR-004: Declare typed questions

## Description

The Sapho question boundary SHALL preserve the configured answer space and option order.

## Inputs

The typed inputs and parameters named in Behavior. Caller-owned values are validated at the crate boundary.

## Outputs

The declared typed result, or a structured SaphoError. Runtime failures carry a partial execution trace.

## Behavior

Questions is an ordered vector of named questions with unique non-empty IDs. Boolean questions carry instructions and true/false criteria; Choice carries an ordered list of at least two distinct labels and descriptions; Score carries an ordered list of at least two level descriptions indexed from zero. Descriptions and instructions are text. The backend request owns a Record state, named questions and a requested model string supplied by its backend binding. Literal config and native code can both construct Questions. Unknown fields are rejected; no dynamic language or expression evaluator is embedded.

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| FR-004-AC-1 | Boolean, Choice and Score question definitions round-trip without changing criteria order. | Test (TC-004) |
| FR-004-AC-2 | Repeated question IDs, repeated choice labels, fewer than two choices/score levels and blank labels are rejected. | Test (TC-004) |
| FR-004-AC-3 | A code primitive can construct a later question using the value of an earlier answer. | Test (TC-004) |

## Dependencies

- [FR-001](FR-001-validate-typed-values.md)
