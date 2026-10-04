# Campaign extraction implementation evidence

2026-10-04: specs FR-049–052 and IT-007, plus base/integrity/scope/dependency reviews
validate with Quire. Common fenced storage, lifecycle, adapter, controls, bounded
recording session, native graph runner and reference-only export are implemented.
The separate Ratatui crate is attached through snapshot/control callbacks. Existing
ix-cli-kit Sapho CLI exposes campaign commands.

Eight library tests and one end-to-end CLI test pass. Clippy with warnings denied
passes for all three affected crates. No claim of completed EARS adaptation,
verified migration, live generic replay or final formal review is made.

Remaining acceptance: EARS consumer uses shared storage/lifecycle/execution/TUI;
explicit migration preserves historical evidence; capture faults/serialization and
recovered sealing have end-to-end coverage; actual TUI interaction/cleanup tests;
provider/exact replay tests; requirements matrix and formal code review; final
handoff to draco:EARS:qwen and draco:Sapho:mod with verified Git/install/run refs.

Rollout warning evidence on Draco: /Users/peter/.cargo/bin/quire reports 0.36.1
(engine 0.50.1); /Users/peter/.nvm/versions/node/v24.15.0/bin/quoin reports 0.28.1.
Command: quire validate --scope . 'spec/modules/campaign/**/*.md'
'spec/reviews/campaign-*.md' in /Users/peter/dev/sapho-campaign-provider exits 0.
Expected: structural validation without duplicate module declarations. Actual
stderr includes DuplicateArchetype for ADR/Plan/Review/SpecReview/Standard from
spec-artifacts-process twice, DuplicateInverseEdge for part_of (aggregates,
contains), and semantic.inline-data-schema in that module. Validation passes;
this warning is reported, not treated as a proven historic failure or ignored.
No old binary/skill fallback was used.
