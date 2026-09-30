---
id: SR-003
title: "Sapho scope-boundary specification review"
type: SpecReview
analysis: scope-boundary
scope: "spec/spec.md and all spec/modules artifacts"
review_set: subset
---
# SR-003: Sapho scope-boundary review

## Summary

Reviewed every module allocation. Core owns types and ports; graph owns compile-time wiring; runtime owns scheduling/logic; Jev owns SDK translation; recording owns exchange retention and exact matching. EARS implementation and review enforcement remain external.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | No findings (placeholder). | spec/spec.md |

## Dispositions

No corrective work required. External contracts are enumerated in the allocation section.

## Verdict

Pass for implementation after the recorded specification corrections and Quire validation. Test cases are planned evidence, not completed evidence.

## Responsibility Allocation

| Requirement | Owning crate | Class |
|-------------|--------------|-------|
| FR-001 | sapho-core | core |
| FR-002 | sapho-core | core |
| FR-003 | sapho-core | core |
| FR-004 | sapho-core | core |
| FR-005 | sapho-core | core |
| FR-006 | sapho-core | core |
| FR-007 | sapho-graph | core |
| FR-008 | sapho-graph | core |
| FR-009 | sapho-graph | core |
| FR-025 | sapho-jev | infrastructure |
| FR-026 | sapho-jev | infrastructure |
| FR-018 | sapho-runtime | core |
| FR-019 | sapho-runtime | core |
| FR-020 | sapho-runtime | core |
| FR-021 | sapho-runtime | core |
| FR-022 | sapho-runtime | core |
| FR-023 | sapho-runtime | core |
| FR-024 | sapho-runtime | core |
| FR-027 | sapho-recording | infrastructure |
| FR-028 | sapho-recording | infrastructure |
| FR-029 | sapho-recording | infrastructure |
| FR-010 | sapho-runtime | core |
| FR-011 | sapho-runtime | core |
| FR-012 | sapho-runtime | core |
| FR-013 | sapho-runtime | core |
| FR-014 | sapho-runtime | core |
| FR-015 | sapho-runtime | core |
| FR-016 | sapho-runtime | core |
| FR-017 | sapho-runtime | core |
| NFR-001 | sapho-core | cross-cutting |
| NFR-003 | sapho-core | cross-cutting |
| NFR-004 | sapho-graph | cross-cutting |
| NFR-002 | sapho-runtime | cross-cutting |
| StR-001 | sapho-core | core |
| StR-002 | sapho-graph | core |
| StR-005 | sapho-jev | core |
| StR-004 | sapho-runtime | core |
| StR-006 | sapho-recording | core |
| StR-003 | sapho-runtime | core |

## External Contracts

| Dependency | Trust | Evidence |
|------------|-------|----------|
| Host Rust primitives | Assumed cooperative/native behavior; checked port outputs | IT-001 and IT-002 |
| Jev SDK protocol | Guaranteed adapter translation via injected transport | IT-003 |
| Hosted model judgment correctness | Assumed; no quality claim | Consumer labelled evaluation |
| Recording filesystem path | Caller-owned; exclusive create and bounded read/write | TC-027 and TC-028 |
| Rust 1.98.1 | Build requirement | NFR-004 |
