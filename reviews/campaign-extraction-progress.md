# Current increment scope

2026-10-04: per-request pure admission measurement and explicit lossless prompt
framing are implemented, along with provider-independent typed invocation receipts
and the dashboard freshness/assessment-denominator extensions. These are generic
APIs; whole-classification answer-dependent planning remains downstream work and
no inference launch is authorized by this increment. Historical receipts below
retain their original scope and are not current completion counts.

# Campaign extraction implementation evidence

2026-10-04: specs FR-049–052 and IT-007, plus base/integrity/scope/dependency reviews
validate with Quire. Common fenced storage, lifecycle, adapter, controls, bounded
recording session, native graph runner and reference-only export are implemented.
The separate Ratatui crate is attached through snapshot/control callbacks. Existing
ix-cli-kit Sapho CLI exposes campaign commands.

Fourteen library tests and two end-to-end CLI tests pass. Clippy with warnings denied
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

Subsequent progress: explicit snapshot migration preserves generic events, job
identities, attempt IDs/parent links and artifact hashes. The stock CLI refuses
unknown domain-owned tables before creating a migration destination. Trusted
admission/dispatch extension hooks commit generic and domain transitions together;
fault injection proves rollback leaves no dispatch intent. EARS now delegates
storage, exact recording sessions and TUI rendering; shared lifecycle conversion
and owning-adapter migration remain unfinished. No completion handoff has been sent.

2026-10-04 migration acceptance: shared snapshot migration now lets trusted
adapters supply domain-only references from the same read transaction, validating
all hashes before creating the destination. EARS pins d66c9c4 and implements
explicit shared-v1 and legacy-v1 imports. Its 23 tests and strict Clippy pass;
policy-history blobs, IDs, auxiliary roles, pause state and retry lineage survive.
Unknown schema, queued controls and missing history refuse before output. The
original synthetic smoke migrated successfully and replays the native full report
byte-identically (SHA3f8e9c56971dadf62832979334c5f5d627d759632d74fb06925256172a97a24a).
No campaign-newness or human-usefulness claim follows. Compiled adapter/runner
integration, full acceptance coverage, actual TUI interaction, final review and
completion handoff are still pending. Relay caller placement remains unverified;
Redis and Draco watcher are healthy according to the actual doctor output.
