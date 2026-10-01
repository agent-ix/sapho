---
id: SR-014
title: "User guide Rust examples and documentation review"
type: SpecReview
analysis: code-review
scope: "sapho@dc25832ca10552ef033a207217ef2bc4a4b8d14e; complete docs/user-guide PR content: README.md, docs/user-guide.md, docs/images/generate.py, all three PNG/SVG pairs; reference-only source: src/lib.rs, Cargo.toml, CLAUDE.md, AGENTS.md, CONTRIBUTING.md, Makefile, crates/sapho-core/src/{ports,model,distribution,value,error}.rs, crates/sapho-graph/src/{spec,compile}.rs, crates/sapho-runtime/src/{engine,operators,trace}.rs, crates/sapho-recording/src/lib.rs, crates/sapho-jev/src/lib.rs; owning core/graph/runtime/logic/Jev/recording specs and cached TypeSafe SDK 0.6.2 public client/config/env/http contracts"
review_set: subset
---
# User documentation and Rust example review

## Summary

Applied rust-review before authoring and formally to the committed documentation diff, under Sapho's own Rust conventions. The README now explains the user's problem, features, logic policy and integration flow; the linked guide supplies complete offline, model-question and weighted-combination graphs, extension examples and operational reference. Three original PNG diagrams have SVG counterparts and a renderer. No engine, dependency, CI, test or normative behavior change.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the scoped documentation and Rust snippets. Examples use the real public compiler, executor, primitive registry and recording/replay implementation. Private executable helpers use Result propagation; no public error API, production panic, unsafe, compatibility reader, credential, copied dependency or private example is introduced. The Jev helper compiles against SDK 0.6.2; its Client constructor correctly returns Client directly. No provider or live model was called, and no model accuracy claim is made.

The diagrams distinguish model probabilities, heuristic degrees and Boolean decisions. Min/max/weighted mean are reducer parameters, comparators are explicit, and the mean shown is 0.65 for inputs 0.8/0.4/0.6 with weights 2/1/1. PNG/SVG text was visually inspected for clipping and legibility. The centered logo display width is 240 pixels; the original logo asset is unchanged. Image metadata and the renderer identify AGPL-3.0-or-later licensing.

## Dispositions

FND-001 accepted-no-change: no substantive findings. Temporary verification harness code is not shipped; it references the authoritative local Sapho checkout through path dependencies. Existing user image files are untouched.

## Verification

- Extracted the documented executable snippets and complete TOML graphs into a temporary consumer harness; compiled and ran them offline against the current Sapho source.
- The default-feature quick start prints needs_review: Boolean(true). Behavioral assertions check the 0.8 equality boundary, all three documented reductions and their decisions, a registered Rust primitive on empty/nonempty text, the Boolean model graph through a deterministic backend seam, recording JSON reload, matching replay and changed-input ReplayMiss.
- Guard/coalesce assertions check skipped execution and both Boolean execution outcomes. The Jev setup helper is compiled, never invoked.
- Local Markdown links and anchors resolve. GitHub's Markdown renderer retains the centered 240-pixel logo, Discord badge and embedded PNG images. SVG XML and Python renderer syntax are checked.
- cargo fmt --all -- --check and git diff --check pass. Complete repository gates are run before opening and again before merging the documentation PR; final results are recorded in the PR validation.
