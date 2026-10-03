---
id: SR-032
title: "Documentation review of the README and how-it-works revamp"
type: SpecReview
analysis: code-review
scope: "sapho docs/readme-revamp versus docs/reference-pages@0ba874a5b32b863dd76e5b7fddbf3dc2d5600253; README.md after the How it works opening, docs/how-it-works.md, docs/improve-a-rule.md, docs/user-guide.md, docs/index.md, docs/examples.md, docs/images/diagrams.py and its 14 SVGs, examples/graphs/{code-review,code-review-relaxed,requirement-check}.yaml, examples/data/code-review-*.json, examples/data/requirements/*.json, examples/recordings/*.json, scripts/record-examples.sh, scripts/check_docs.py, crates/sapho-select/examples/acquisition.rs; reference-only source: crates/*/src, spec/spec.md, spec/modules/core/functional/FR-005"
review_set: subset
---
# SR-032: README and how-it-works documentation review

## Summary

Reviewed the documentation rewrite for factual accuracy against the code, the
specification, the example graphs and the recorded Jev answers. Every offline
command shown was run from the repository root with no credential, and every
documented number was compared with replay, measure and tune output. Fifteen
findings; fourteen fixed in this change, one accepted.

## Verdict

PASS after fixes. No engine, API, specification or dependency change. Model
answers in the examples are real Jev 1.13.0 responses recorded once; the
labels are synthetic tutorial labels and the pages make no accuracy claim.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | medium | `min`, `max` and `weighted_mean` were listed as operations; they are `reducer` values of `reduce`, which also needs `empty`. | docs/how-it-works.md |
| FND-002 | medium | The link check ignored `src`/`srcset` paths in `<picture>` blocks. | scripts/check_docs.py |
| FND-003 | medium | Several documented values and every figure value were unpinned; figure values were typed by hand. | scripts/check_docs.py, docs/images/diagrams.py |
| FND-004 | low | Findings were said to always point back to their source; sources exist only when inputs carry them. | docs/how-it-works.md |
| FND-005 | low | Probability outputs were called strengths in the README. | README.md |
| FND-006 | low | The replay match list omitted the expected model. | docs/improve-a-rule.md |
| FND-007 | low | A user-guide link pointed at the wrong how-it-works section. | docs/user-guide.md |
| FND-008 | low | Score answers also carry a confidence; the table implied only choice answers do. | docs/how-it-works.md |
| FND-009 | low | The distribution policy was described as governing all answers; it governs complete distributions only. | docs/how-it-works.md |
| FND-010 | low | `sapho record` takes one input per run; the page implied it reads the dataset. | docs/improve-a-rule.md |
| FND-011 | low | `record` and `export-training` refuse existing output paths; the page did not say so. | docs/improve-a-rule.md |
| FND-012 | low | The README understated the relaxed rule's change and omitted `--recording` for `record`. | README.md |
| FND-013 | low | The note that re-recording moves answers had no stated evidence. | docs/improve-a-rule.md |
| FND-014 | low | Figure code paraphrased question text, duplicated alt text, rounded tick labels to one decimal and skipped the top-edge bounds check. | docs/images/diagrams.py |
| FND-015 | low | The selection example's file count is tied to the number of graphs in `examples/graphs`. | crates/sapho-select/examples/acquisition.rs |

## Dispositions

- FND-001 to FND-012 fixed as described. For FND-003, `diagrams.py` reads every
  model answer from the recordings, and `recorded()` in `check_docs.py` pins
  the documented replay, measure, tune, trace-usage, exchange-count, split and
  CLI-default values.
- FND-013 fixed with first-hand evidence. `scripts/record-examples.sh` was run
  a second time during this review; requests matched and answers moved by up to
  0.06. The original recordings were restored and the sentence states what
  was observed.
- FND-014 fixed: figures use the recorded question instructions, tick labels
  print exactly, and the bounds check covers the top edge. Each Markdown `alt`
  text is the figure's own `aria-label`; the pair is kept because Markdown
  cannot read an SVG's label.
- FND-015 accepted-no-change. The count is shown to readers ("selects seven
  files") and changes only when an example graph is added.

## Verification

- `make ci` and `make docs-check` pass on the final tree.
- `check_docs.py` replays both code-review graphs, the five requirement
  statements and the dataset measurements and tuning, and asserts the
  documented values. A deliberately broken `srcset` fails the link check.
- `python3 docs/images/diagrams.py` regenerates all 14 SVGs byte-identically.
  Each figure was rendered on #ffffff and #0d1117 and inspected for clipping,
  overlap and contrast; GitHub's Markdown renderer selects the dark figure
  under a dark color scheme.
- `scripts/record-examples.sh` was run end to end against live Jev.
