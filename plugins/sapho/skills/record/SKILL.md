---
name: record
description: Capture explicit local Sapho evaluation evidence and verify exact offline replay, including partial failures.
license: AGPL-3.0-or-later
---
<!-- SPDX-License-Identifier: AGPL-3.0-or-later; Copyright (C) 2026 Agent-IX -->

Read the [CLI guide](https://github.com/agent-ix/sapho/blob/main/docs/cli-guide.md), especially binding metadata, limits and replay identity. Confirm installed syntax with `sapho record --help` and `sapho replay --help`.

Validate/inspect the requested graph, select explicit input/context and choose fresh local evidence destinations. For private application state, keep inputs, traces and recordings in the user's chosen private data location. Never move private examples into a public repository.

Capture through the public host:

```sh
sapho record GRAPH.yaml --input INPUT.json --recording NEW-SAVED.json --trace NEW-TRACE.json --output NEW-REPORT.json
```

Add explicit `--bindings BINDINGS.yaml` for an authorized live provider, or use a custom Rust host with injected backends. Set finite work/data/artifact ceilings appropriate to the requested batch. Graph/binding files contain model metadata and policy, never credentials. No implicit retry or overwrite occurs.

Record retains successful validated raw exchanges. A failed run retains partial trace and any earlier completed exchanges; inspect that evidence without manufacturing missing answers or treating absence as a pass. Preserve raw answers, their declared policy and actual model identity.

Replay with the same graph/input and saved recording:

```sh
sapho replay GRAPH.yaml --input INPUT.json --recording NEW-SAVED.json
```

Use `--typed-input` in both invocations for typed selector inputs. Replay never consults credentials or delegates live. Model, policy, state and ordered questions are exact identity. Conflicting binding tuples refuse; an absent name in a partial/empty recording needs explicit matching binding metadata. Changed requests produce ReplayMiss. Threshold-only experiments can reuse evidence when requests remain identical.

Report capture/replay results, failures and paths. Saved responses are proposals and cannot be promoted automatically to correctness labels. The skill does not install hooks, repair loops or agent-behavior enforcement, and follows the user's existing authorization for calls and writes.
