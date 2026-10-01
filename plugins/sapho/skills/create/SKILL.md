---
name: create
description: Create and validate Sapho YAML or JSON evaluation graphs from requested data, model questions and explicit logic.
license: AGPL-3.0-or-later
---
<!-- SPDX-License-Identifier: AGPL-3.0-or-later; Copyright (C) 2026 Agent-IX -->

Build the requested Sapho graph with explicit input/output meaning. Use YAML for hand-authored graphs or JSON for generated programs. Follow the workspace's specification and review policy before implementation.

Read the [CLI guide](https://github.com/agent-ix/sapho/blob/main/docs/cli-guide.md) and the relevant [graph semantics](https://github.com/agent-ix/sapho/blob/main/docs/user-guide.md). Prefer these authoritative references over reproducing a second evaluator. Use `sapho --help` and subcommand help to confirm installed syntax.

Choose units and context, then state which outputs are Boolean decisions, model Probabilities or heuristic Degrees. Wire model questions, native transformations and explicit combiners as an acyclic graph. Use record/list assembly for data-only context and combinations; map/filter/pairs/join for identified collections; Boolean guards and explicit Optional defaults for conditional work.

Keep questions and thresholds in configuration. A weighted reducer needs matching item IDs: map values and weights from the same identified records, or accept explicit typed inputs. Independently decoded plain lists have distinct occurrence identities.

A `code` node requires the caller's registered Rust primitive. Stock Sapho has no domain extractor registry; use a custom `sapho-cli::Runner` host when needed. Never replace an unavailable primitive with a passing stub or load scripts into the graph.

Run `sapho validate GRAPH.yaml` and `sapho inspect GRAPH.yaml` before evaluation. Exercise small original offline cases using `sapho run GRAPH.yaml --input INPUT.json`. Use `--typed-input` for selector output and `--fail-on OUTPUT` only for a Boolean finding. Exit 1 is that explicit finding; exit 2 is refusal.

Deliver the graph, its input example and the output interpretation. Live models, writes and publication follow the user's existing authorization. No hook or agent-enforcement installation is part of graph creation.
