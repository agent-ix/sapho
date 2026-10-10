---
id: SR-064
title: "SAPHO-23 candidate sweep specification review"
type: SpecReview
analysis: base
scope: "FR-079, FR-080 and IT-016 against current origin/main"
review_set: subset
---
# SR-064: SAPHO-23 base review

## Summary

Reviewed `sapho tune`'s explicit ordered candidates and default 16 ceiling, GraphSpec literal Datum identity and strict parser, GraphArtifact.source identity, exact replay and current CLI artifact I/O on `origin/main` `e77b81eddd90b606561f2ed606cde74f133ea937`. This is specification evidence only; candidate generation has not been implemented.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | Generating inside `tune` would conflict with FR-037's explicit candidate contract and development-only scope. | FR-079, FR-080 |
| FND-002 | high | A late invalid grid coordinate could leave earlier candidate files written. | FR-079, IT-016 |
| FND-003 | medium | Literal IDs are not guaranteed unique across a GraphSpec; ambiguous targets could mutate more than one location. | FR-079 |
| FND-004 | medium | List-weight replacements could silently change item identity or source attribution. | FR-079, IT-016 |
| FND-005 | medium | Existing destinations or write failures could overwrite or expose incomplete candidate sets. | FR-080, IT-016 |
| FND-006 | medium | Graph metadata fields would violate the closed GraphSpec schema, while graph/source identities have separate definitions. | FR-080 |
| FND-007 | medium | A downstream-only sweep replays exactly, but a request-affecting literal can still produce ReplayMiss. | FR-080, IT-016 |

## Dispositions

FND-001: a separate `sapho sweep` emits explicit candidate paths; FR-037 and `tune` remain unchanged. FND-002: the entire grid is checked, compiled and serialized under byte ceilings before directory creation. FND-003: every axis must resolve to exactly one literal occurrence. FND-004: numeric list replacements preserve item IDs, order and SourceRefs. FND-005: publication requires an exclusive new directory/files, writes the manifest last and cleans only its own files on failure. FND-006: a separate versioned manifest binds ordinal filenames to coordinate values and raw-byte SHA-256 digests without changing GraphSpec or GraphArtifact.source. FND-007: the test pairs downstream-only replay success with exact request-affecting ReplayMiss.

## Verdict

Ready for planner review of the CLI, grid, identity and publication contract. `quire validate --scope . 'spec/**/*.md'` exited 0; `quire matrix --scope .` exited 0 and showed all ten new ACs untagged; `git diff --check` exited 0. `filament open` could not dispatch the IDE URL through the OS opener (error -10661). IT-016 is planned, not executed implementation evidence. Dataset serde/validate and EARS production boundary remain outside this change; the separate EARS dataset-config repair is not touched.
