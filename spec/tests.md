---
id: TM-001
title: "YAML/JSON, data CLI and graph workflow test matrix"
type: TestMatrix
---
# TM-001: YAML/JSON, data CLI and graph workflow test matrix

This matrix indexes the SAPHO-3/SAPHO-5 implementation scope. Detailed scenarios remain in their owning TC files. Status records the passing complete workspace test lanes; consumer-owned EARS migration is tracked separately by SAPHO-4. No live model accuracy is claimed.

## Functional Requirement Coverage

| Functional Req | Acceptance Criteria | Test Cases | Status |
|---|---|---|---|
| FR-007 | FR-007-AC-1 | TC-007 | ✅ verified offline |
| FR-007 | FR-007-AC-2 | TC-007 | ✅ verified offline |
| FR-007 | FR-007-AC-3 | TC-007 | ✅ verified offline |
| FR-030 | FR-030-AC-1 | TC-030 | ✅ verified offline |
| FR-030 | FR-030-AC-2 | TC-030 | ✅ verified offline |
| FR-030 | FR-030-AC-3 | TC-030 | ✅ verified offline |
| FR-031 | FR-031-AC-1 | TC-031 | ✅ verified offline |
| FR-031 | FR-031-AC-2 | TC-031 | ✅ verified offline |
| FR-031 | FR-031-AC-3 | TC-031 | ✅ verified offline |
| FR-032 | FR-032-AC-1 | TC-032 | ✅ verified offline |
| FR-032 | FR-032-AC-2 | TC-032 | ✅ verified offline |
| FR-032 | FR-032-AC-3 | TC-032 | ✅ verified offline |
| FR-033 | FR-033-AC-1 | TC-033 | ✅ verified offline |
| FR-033 | FR-033-AC-2 | TC-033 | ✅ verified offline |
| FR-033 | FR-033-AC-3 | TC-033 | ✅ verified offline |
| FR-034 | FR-034-AC-1 | TC-034 | ✅ verified offline |
| FR-034 | FR-034-AC-2 | TC-034 | ✅ verified offline |
| FR-034 | FR-034-AC-3 | TC-034 | ✅ verified offline |
| FR-035 | FR-035-AC-1 | TC-035 | ✅ verified offline |
| FR-035 | FR-035-AC-2 | TC-035 | ✅ verified offline |
| FR-035 | FR-035-AC-3 | TC-035 | ✅ verified offline |
| FR-036 | FR-036-AC-1 | TC-036 | ✅ verified offline |
| FR-036 | FR-036-AC-2 | TC-036 | ✅ verified offline |
| FR-036 | FR-036-AC-3 | TC-036 | ✅ verified offline |
| FR-037 | FR-037-AC-1 | TC-037 | ✅ verified offline |
| FR-037 | FR-037-AC-2 | TC-037 | ✅ verified offline |
| FR-037 | FR-037-AC-3 | TC-037 | ✅ verified offline |
| FR-038 | FR-038-AC-1 | TC-038 | ✅ verified offline |
| FR-038 | FR-038-AC-2 | TC-038 | ✅ verified offline |
| FR-038 | FR-038-AC-3 | TC-038 | ✅ verified offline |
| FR-039 | FR-039-AC-1 | TC-039 | ✅ verified offline |
| FR-039 | FR-039-AC-2 | TC-039 | ✅ verified offline |
| FR-039 | FR-039-AC-3 | TC-039 | ✅ verified offline |
| FR-040 | FR-040-AC-1 | TC-040 | ✅ verified offline |
| FR-040 | FR-040-AC-2 | TC-040 | ✅ verified offline |
| FR-040 | FR-040-AC-3 | TC-040 | ✅ verified offline |
| FR-041 | FR-041-AC-1 | TC-041 | ✅ verified offline |
| FR-041 | FR-041-AC-2 | TC-041 | ✅ verified offline |
| FR-041 | FR-041-AC-3 | TC-041 | ✅ verified offline |
| FR-042 | FR-042-AC-1 | TC-042 | ✅ verified offline |
| FR-042 | FR-042-AC-2 | TC-042 | ✅ verified offline |
| FR-042 | FR-042-AC-3 | TC-042 | ✅ verified offline |

## Test Case Summary

| Test ID | Title | Type | Priority | Traces To | Status |
|---|---|---|---|---|---|
| TC-007 | Load declarative graph configuration | Integration | P1 | FR-007-AC-1, FR-007-AC-2, FR-007-AC-3 | ✅ verified offline |
| TC-030 | Assemble records from graph values | Integration | P1 | FR-030-AC-1, FR-030-AC-2, FR-030-AC-3 | ✅ verified offline |
| TC-031 | Assemble ordered identified lists | Integration | P1 | FR-031-AC-1, FR-031-AC-2, FR-031-AC-3 | ✅ verified offline |
| TC-032 | Decode plain data by a declared type | Integration | P1 | FR-032-AC-1, FR-032-AC-2, FR-032-AC-3 | ✅ verified offline |
| TC-033 | Inspect a graph without executing it | Integration | P1 | FR-033-AC-1, FR-033-AC-2, FR-033-AC-3 | ✅ verified offline |
| TC-034 | Invoke bounded graphs from data | Integration | P1 | FR-034-AC-1, FR-034-AC-2, FR-034-AC-3 | ✅ verified offline |
| TC-035 | Record and replay CLI evaluations | Integration | P1 | FR-035-AC-1, FR-035-AC-2, FR-035-AC-3 | ✅ verified offline |
| TC-036 | Measure declared outputs against labelled cases | Integration | P1 | FR-036-AC-1, FR-036-AC-2, FR-036-AC-3 | ✅ verified offline |
| TC-037 | Compare bounded development candidates | Integration | P1 | FR-037-AC-1, FR-037-AC-2, FR-037-AC-3 | ✅ verified offline |
| TC-038 | Export curated training cases | Integration | P1 | FR-038-AC-1, FR-038-AC-2, FR-038-AC-3 | ✅ verified offline |
| TC-039 | Select bounded attributable files | Integration | P1 | FR-039-AC-1, FR-039-AC-2, FR-039-AC-3 | ✅ verified offline |
| TC-040 | Select explicit Git changes | Integration | P1 | FR-040-AC-1, FR-040-AC-2, FR-040-AC-3 | ✅ verified offline |
| TC-041 | Select typed values from structured data | Integration | P1 | FR-041-AC-1, FR-041-AC-2, FR-041-AC-3 | ✅ verified offline |
| TC-042 | Package graph authoring workflows | Integration | P1 | FR-042-AC-1, FR-042-AC-2, FR-042-AC-3 | ✅ verified offline |
