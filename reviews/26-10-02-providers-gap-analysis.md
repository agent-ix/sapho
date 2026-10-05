---
id: SR-027
title: "Sapho repository traceability audit with CLM providers"
type: SpecReview
analysis: gap-analysis
scope: "sapho@85a14b5b4a319d383aa10e2f18ac57f2a1e8158a; whole spec/, crates/, src/, tests/, plugins/sapho/, public CLI catalog, package features and existing guides; repository-driven, planless"
review_set: subset
relationships:
  - { target: "ix://agent-ix/sapho/StR-011", type: references }
---
# SR-027: Repository traceability audit

## Summary

Audited Sapho's complete source/spec/test surface, including unchanged behavior, through the computed matrix, reverse ownership and stub checks. This repository-driven audit does not assess a plan or claim Decisions delivery.

## Verdict

PASS for repository traceability, ownership and non-hollow implementation checks. No untagged criterion, ignored-only evidence, unspecified behavior family or source/test stub was found. This verdict does not claim live CLM quality, provider deployment, account access, stakeholder acceptance or independent semantic assurance.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Coverage

- Repository root: isolated Sapho worktree on the development host; spec root spec/; identity ix://agent-ix/sapho. The master specification has no artifact id, so the relationship names the provider stakeholder obligation rather than inventing a root id.
- Reconciliation: `quire matrix --scope . --strict --format json`, quire 0.36.1 / engine 0.50.1; no run-evidence store read.
- Plan completion: not assessed
- Criteria: tagged 149; untagged 0; tagged-by-ignored-test 0; method-without-symbol 3. The three are FR-047-AC-1/2 (Decisions contract/access inspection) and NFR-001-M-1 (dependency/safety inspection). They are not represented as passing inference tests. No fallback grep matrix was used.
- Evidence: actual `make ci` exit 0 at the reviewed source, 84 tests and one doctest in each feature lane; no fabricated evidence binding or coverage percentage.
- Inventory: 13 behavior families below, covering public APIs, CLI commands/configuration and plugin workflows. Untraced behavior families 0; source stubs 0; test stubs 0.
- Semantic review: skipped; the optional independent requirement-by-requirement semantic audit was not requested. Code/Rust review separately checked the new tests' actual boundaries and assertions.

## Reverse Ownership Inventory

| Behavior family | Owning requirements |
|---|---|
| Core values, provenance, IDs, primitives and registries | FR-001..003, FR-006, NFR-001 |
| Typed questions, model answers, policy and plain data | FR-004..006, FR-032 |
| YAML/JSON loading, bounded compiler and conditional ports | FR-007..009 |
| Bounded execution, map/filter/pairs/join/collect and trace identity | FR-010..017, FR-030..031, NFR-002 |
| Crisp/heuristic/probability operators | FR-018..024 |
| Shared source-free System One request translation | FR-025, FR-043; systemone allocation |
| Jev HTTP/SDK adapter and provider failures | FR-025..026, FR-045 |
| CLM request/response translation, limits and raw usage | FR-043..044, IT-006 |
| Recording, validation and exact offline replay | FR-027..029, FR-035, FR-045 |
| CLI inspection, invocation, acquisition, shared credentials/streams/exits/config/version | FR-033..035, FR-045..046, NFR-005 |
| Dataset measurement, development tuning and training export | FR-036..038 |
| File/Git/JSON selection and process cleanup | FR-039..041, NFR-005 |
| Original plugin workflows and optional embedding facade | FR-042 and the owning exported crate requirements; facade owns no independent semantics |

Public knobs are allocated to these families: runtime limits, selector ceilings, graph format, model/expected-model/distribution policy, CLM transport limits/base endpoint, host credentials and CLI finding-selection/result policy. Existing finite guards and identity checks follow the owning robustness/integrity requirements. Original synthetic examples remain local and consumer source/evidence was not imported.

## Integrity and External Prerequisite

Source and tests contain no TODO/unimplemented production paths, no meaningful hollow return or test-only production bypass. Abstract ports have concrete native/backend implementations; facade re-exports deliberately expose existing behavior. The new tests exercise real encoding/decoding and transport/recording/command boundaries. No percentage-based coverage claim is made.

SAPHO-16 remains pending because the official Decisions announcement does not expose a verified usable preview contract to this account. FR-047 allocates that prerequisite as inspection and forbids inventing a backend. SAPHO-14 and SAPHO-15 cover the independently implemented CLM/shared CLI scope.

## Executed Handoff Gate

The second full `make ci` passed at 5be5e5cf0691def282c74215f9cd37f4e4821050, including validation of this artifact. Both feature lanes ran 84 tests plus one doctest; the release build passed separately. No source or test behavior changed after the reviewed implementation revision.
