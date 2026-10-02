---
id: SR-021
title: "README workflow and Rust example review"
type: SpecReview
org: agent-ix
analysis: code-review
scope: "sapho@f3253bd28ed170879227da826d5953695858584d versus main@13589f54f08629e9385c0b477137eb0e1edc024c; changed README.md including all code blocks and links; read-only references: AGENTS.md, CLAUDE.md, CONTRIBUTING.md, Cargo.toml, crates/sapho-cli/Cargo.toml, rust-toolchain.toml, rustfmt.toml, Makefile, .github/workflows/{ci,cla}.yml, src/lib.rs, crates/sapho-cli/src/{main,args,host,bindings,command}.rs, crates/sapho-cli/tests/invocation.rs, crates/sapho-graph/src/spec.rs, crates/sapho-runtime/src/engine.rs, crates/sapho-evidence/src/lib.rs, docs/{cli-guide,user-guide}.md, examples/graphs/{review,review-conservative,combine,files,multilayer}.yaml, examples/data/{combine-input,review-dataset}.json, plugins/sapho/plugin.json and skills/{create,tune,record}/SKILL.md; official Claude Code plugin manifest, creation and installation documentation"
review_set: subset
---
# README workflows and Rust example review

## Summary

Reviewed the complete README rewrite and its runnable examples against current source and Sapho's Rust conventions. The README now explains compound decisions, questions, combinations and layers before presenting CLI installation, YAML authoring, workflows, Jev setup, Rust integration and Claude Code skills.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Verdict

PASS for the documentation and Rust example. The executable uses typed values, deterministic maps, borrowed inputs, Result propagation and finite default run limits. Its boxed error is confined to the example executable's entry point. It introduces no public error API, panic, unsafe code, blocking host work on an async worker, fallback reader or engine change. The original user-supplied introduction and existing centered logo and diagrams are retained.

CLI instructions match the checked-in arguments and real behavior. Stock CLI operations and custom Rust-host responsibilities are stated separately. Recording/replay instructions preserve exact request identity; tuning uses supplied development labels and a separate held-out evaluation. Tutorial data and heuristic strengths are identified as such. Local model integration requires a registered backend; there is no claim of a bundled Laya or KEV transport.

The Claude Code instructions use the existing plugin directory directly. Claude Code 2.1.284 validates its components and lists `sapho@inline` as loaded. The README explicitly describes session loading and repeating the flag. No marketplace installation, plugin publication, hook, credential setup or model invocation was performed.

## Dispositions

FND-001 accepted-no-change: no substantive findings in the final scoped diff.

## Verification

- Extracted the exact README YAML and Rust snippets into temporary files in the owning worktree. The Rust example compiled and printed `needs_review: Boolean(true)` against the actual local engine, without warnings after the documented crate header was added. `make fmt-check` passed.
- Actual CLI runs verified support 0.7/0.8/0.9 gives true/false/false, selected findings exit 1/0, and invalid input exits 2. The weighted example produced strength 0.65 and review true.
- Files and staged-Git selectors fed the included graph successfully using typed input. Measurement selected two development cases; tuning ranked the baseline first at agreement 1.0; export emitted two development rows. These are tutorial controls, not semantic accuracy measurements.
- An offline capture using the extracted threshold graph and its exact replay produced equal complete reports. No live provider, model or trainer was called. Jev configuration is checked against the owning binding schema and guide; live service behavior is not claimed verified.
- All 25 local README links and anchors resolve, all shell code blocks parse, and `git diff --check` passes. Official Claude Code documentation supports optional manifest discovery, directory-derived naming and session loading through `--plugin-dir`.
- Temporary verification files and build artifacts are owned by this worktree and will be removed after delivery. No private research examples, dependency files or third-party assets are copied into the change.
- Full pre-PR and pre-merge repository gates are recorded in the PR validation; no production source, dependency, test, specification or CI lane is modified.
