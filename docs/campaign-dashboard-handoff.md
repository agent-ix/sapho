# Pinned generic dashboard/API battletest

Source API: 54bdaabb238b242295162bb3bd331105915797d6 on
`feat/campaign-local-provider`. No merge is performed. Source review is
[SR-033](../reviews/2026-10-04-request-budget-dashboard-review.md), conditional
for the full campaign release because three broader strict-matrix gaps remain.

Executable built from that revision:
`/Users/peter/dev/sapho-campaign-provider/target/handoff/54bdaabb238b242295162bb3bd331105915797d6/sapho`.
SHA256: `e335477dee4a1fc105a8e77ac2693c7f07526fc6ef24700078aad75526e5a8a8`.
The adjacent `receipt.json` and `dashboard-v1-synthetic.json` record the exact
build/test scope and a source-free synthetic CLI output. The binary does not
embed a Git revision; the build receipt and executable hash pin it. Build began
from a clean tree; the untracked review document at completion is not compiled.

```sh
BIN=/Users/peter/dev/sapho-campaign-provider/target/handoff/54bdaabb238b242295162bb3bd331105915797d6/sapho
STATE=/private/tmp/sapho-dashboard-battletest-20261004-54bdaab
"$BIN" campaign --state "$STATE" dashboard
```

The CLI smoke initialized this new synthetic state and decoded schema 1 with
null last-event time, live observation, completion and selected attempt. Requesting
absent attempt 1 exits 2; subsequent projection has unchanged durable sequence.
No source payload was imported, inference started or runtime configuration changed.
The [v1 contract](campaign-dashboard-api.md) includes actual assessment denominator
and provenance separately from a goal, and authoritative last-event time separately
from projection generation time. Domain consumers remain downstream; do not poll
private payloads or SQLite or parse terminal display text.

All-feature workspace tests pass (134 including doctests), minimal-feature tests
pass (132 including doctests), strict all-target Clippy passes both feature lanes,
and strict docs, cargo-deny, formatting and unsafe audit pass. One subprocess helper
is explicitly ignored in each test lane and exercised by its parent capacity test.
The whole strict matrix still has FR-051-AC-3/4 and FR-052-AC-3 untagged. No broad
application completion or inference-launch approval follows from this receipt.

The new [single-request admission API](local-model-admission.md), provider-independent
observations and explicit framed prompt format are reusable library capabilities.
The stock campaign CLI continues its default prompt recipe; downstream hosts must
explicitly pin/select any new recipe. Whole-classification answer-dependent plan
admission remains unfinished downstream. Do not use this receipt to start inference.
