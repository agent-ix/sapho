---
name: tune
description: Measure and tune Sapho logic graphs against explicitly supplied development labels while preserving held-out evaluation.
license: AGPL-3.0-or-later
---
<!-- SPDX-License-Identifier: AGPL-3.0-or-later; Copyright (C) 2026 Agent-IX -->

Read the [CLI guide](https://github.com/agent-ix/sapho/blob/main/docs/cli-guide.md), especially labelled cases, measurement, tuning and training export. Confirm syntax with `sapho tune --help`.

Obtain a dataset with stable case IDs, typed inputs, nonempty Boolean labels by output, declared label provenance, and development/held_out splits. Saved model proposals and unlabelled score distributions are evidence, not correctness labels. Describe how labels were obtained; Sapho records that declaration without certifying it.

Measure the baseline explicitly:

```sh
sapho measure GRAPH.yaml --dataset LABELS.json --split development
```

Inspect coverage, errors and per-output denominators before interpreting agreement or Brier score. Only Probability outputs receive Brier scoring; a Degree remains heuristic. Exit 2 includes incomplete/unscored measurement and retains the report.

Author a small bounded ordered candidate set. Make changes to questions, routing, combinations or thresholds explicit and preserve original graphs and evidence. Compare with:

```sh
sapho tune --candidate BASE.yaml --candidate CANDIDATE.yaml --dataset LABELS.json --output-name OUTPUT --metric agreement --output NEW-TUNING.json
```

Use `--metric brier` for a Probability output. Default candidate ceiling is 16. Model work requires explicit `--bindings` or `--replay SAVED.json`. Replay only matches exact requests; changed prompts/context require new authorized capture. Incomplete or replay-incompatible candidates cannot win.

Tuning selects development cases only. After choosing a candidate, use a separate explicitly requested `measure --split held_out` invocation. Report the candidate definitions, evidence, coverage and agreement with supplied labels, including remaining failures. Do not claim semantic accuracy from synthetic examples or model self-agreement.

When requested, `sapho export-training --dataset LABELS.json --output NEW-DEVELOPMENT.jsonl` exports curated development supervision. Downstream trainers own model-specific conversion and actual training; neither this export nor tuning starts a trainer. Follow existing authorization for model calls, evidence writes and publication.
