---
id: SR-033
title: "Spec review of the M1 bundle (SAPHO-75)"
type: SpecReview
analysis: base
scope: "agent-ix/sapho@83980c847c8ed97cef57e56d33fa6b09375d151d; spec/spec.md, spec/modules/core (FR-048, NFR-001, TC-048), spec/modules/evidence (FR-036, FR-038, TC-036, US-008), spec/modules/ollama/**; ticket SAPHO-75"
review_set: subset
---
# SR-033: Spec review of the M1 bundle

## Summary

Ticket: SAPHO-75. PR: agent-ix/sapho#19 at 83980c847c8ed97cef57e56d33fa6b09375d151d. Method: spec-review (reviewer run 764c04b7-2ad7-4bef-b559-3f11ebcafc0c, claude-opus-5-5, quoin 0.28.1, spec-artifacts-process@7b50469a1c31f33e4c6e2d73e3614ca21d064735). Review date 2026-10-05.

Umbrella /quoin:spec-review of the M1 bundle (sapho#19 and the downstream dataset consumer PRs): ticket acceptance criteria, owner rules from the initiative brief and the owner's plan, the author's open items and decisions, and the verdict. Sub-analysis findings are in their own artifacts.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | high | BLOCKER: ollama module specifies no ModelBackend (ask) implementation; SAPHO-75 AC1 unmet | spec/modules/ollama/spec.md:18 |

## Analysis

### FND-001 (high, confidence high, coverage, blocking)

Unit: ollama/spec.md#In-Scope at spec/modules/ollama/spec.md:18. Related: FR-048, FR-049, and in the downstream consumers FR-004, FR-008 (pipeline) and FR-007, FR-009 (question stages).

> The [Extractor](../modules/core/functional/FR-048.md) implementation over Ollama's generate endpoint; explicit generation settings per binding; token estimation before a call and the reported token count after it; a `TooLarge` outcome; one request in flight per process; model identity, digest, usage and raw exchange bytes on every response; embeddings over Ollama's embed endpoint; host-configured endpoint, timeout and byte ceilings.

SAPHO-75 What says the ollama module serves both ask and extract tasks (SAPHO-32: implements both ModelBackend and Extractor), and AC1 requires requirements for every listed behaviour. The module's In Scope, FR-049..053 and the master spec (spec/spec.md:19, 'for extraction and embeddings') cover only Extractor and embeddings. Failure scenario: the downstream pipeline's FR-004/FR-005/FR-008 route every ask task to a model binding with backend `ollama` through ModelBackend, so the M2 exit check (toy ask task over 20 items on Qwen), the downstream question stages have no backend that can answer them; the downstream question-stage integration test cannot be built. The gap is real (author open item a): core Answer::Boolean carries P(true) (sapho-core model.rs, FR-005) while Ollama generate gives crisp text. Options, not decided: (a) an Ollama ModelBackend returning crisp answers as P in {0,1} with the distribution marked unavailable and a stated 'uncalibrated' semantics in core; (b) derive P(true) from Ollama token logprobs (needs an Ollama version with logprobs; verify on the local inference host); (c) run the downstream question stages as `extract` tasks with a fixed answer schema and keep ModelBackend for the fast tier only (changes the downstream pipeline's FR-004/FR-008 and the downstream question stages, and also removes the dynamic-question problem in the confirm finding); (d) add a crisp Answer variant to core. Whichever is chosen, the ollama module and master spec scope must state it before M2.

## Scope

| Unit | Path | Role |
|------|------|------|
| FR-048 | spec/modules/core/functional/FR-048.md | examined |
| NFR-001 | spec/modules/core/non-functional/NFR-001.md | examined |
| spec/modules/core/spec.md | spec/modules/core/spec.md | examined |
| FR-036 | spec/modules/evidence/functional/FR-036.md | examined |
| FR-038 | spec/modules/evidence/functional/FR-038.md | examined |
| US-008 | spec/modules/evidence/usecase/US-008.md | examined |
| FR-049 | spec/modules/ollama/functional/FR-049.md | examined |
| FR-050 | spec/modules/ollama/functional/FR-050.md | examined |
| FR-051 | spec/modules/ollama/functional/FR-051.md | examined |
| FR-052 | spec/modules/ollama/functional/FR-052.md | examined |
| FR-053 | spec/modules/ollama/functional/FR-053.md | examined |
| IT-007 | spec/modules/ollama/integration/IT-007.md | examined |
| spec/modules/ollama/spec.md | spec/modules/ollama/spec.md | examined |
| StR-012 | spec/modules/ollama/stakeholder/StR-012.md | examined |
| US-012 | spec/modules/ollama/usecase/US-012.md | examined |
| spec/spec.md | spec/spec.md | examined |
| FR-005 | spec/modules/core/functional/FR-005-validate-model-answers.md | context_only |
| FR-006 | spec/modules/core/functional/FR-006-define-replaceable-model-backend.md | context_only |
| FR-007 | spec/modules/graph/functional/FR-007-load-declarative-graph-configuration.md | context_only |
| FR-035 | spec/modules/cli/functional/FR-035.md | context_only |

## Verdict

1 high, 0 medium, 0 low; blocking: FND-001.

## Ticket Acceptance Criteria (SAPHO-75)

| AC | Result |
|----|--------|
| `spec/modules/ollama/` has requirements for every Ollama behaviour, quire validate passes | Not met: validate passes, but the `ask` path (ModelBackend) is absent (FND-001). Extract, token counting, too_large, one request at a time, digest and usage are covered by FR-049..053. |
| Core module specifies the Extractor port | Met (FR-048, master spec Public Contract, ErrorCode TooLarge). |
| FR-036 and master spec accept model labels with kind and source, forbid self-confirmation, "never promoted" wording gone | Met for FR-036, FR-038, US-008 and spec.md; residual "proposals, not labels" in FR-035 and docs/cli-guide.md (SR-040 FND-001, low). |
| Master scope lists the Ollama backend and the Extractor port | Met (spec.md In Scope, module table, flowchart). |
| No dependency on the downstream dataset consumer or a dataset module | Met: no mention anywhere; dataset pipelines are explicitly out of scope. |
| One /spec-review with no open blockers | Not met: this review has blockers (see Bundle Verdict). |

SAPHO-57 (schema validation location fixed in spec: core) and the label-provenance ticket M1 AC (Sapho part) are met. SAPHO-32's "implements both ModelBackend and Extractor" is not specified.

## Author Open Items

- (a) Ollama `ask` path: confirmed unspecified. The spec is not coherent without it, because the downstream dataset consumers route their question stages through it; recorded as blocking FND-001 with options.

## Author Decisions

- ExtractResponse gained `model {name, digest}`: confirm. The downstream dataset consumer FR-006 and FR-014 need it. The digest is the manifest digest from /api/tags, which does not identify weights across tags (downstream dataset consumer SR-006 FND-002) and is resolved only once per binding (SR-038 FND-002).
- Bytes-per-token ratio 3.0 and 15 percent tolerance: confirm as starting values. Their evidence depends on what prompt_eval_count means under Ollama's prompt cache (SR-038 FND-001); measure on the local inference host before relying on estimate_exceeded.
## Bundle Verdict

Blockers: yes. Six findings across the three PRs must be fixed in the specifications before M2 implementation starts:

1. sapho SR-033 FND-001: the ollama module specifies no ModelBackend (`ask`) implementation, so the downstream question stages and the M2 exit check have no local-model backend.
2. downstream dataset consumer SR-002 FND-001: confirm needs a per-item question block; Sapho graphs only build literal questions and the downstream dataset consumer library entry point accepts no primitive registry.
3. downstream dataset consumer SR-002 FND-002: an item with a false confirm claim can never leave needs_review.
4. downstream dataset consumer SR-001 FND-001: a reviewer's correction plus its own accept becomes `confirmed`, and model-kind reviewers are admitted (self-confirmation).
5. downstream dataset consumer SR-006 FND-001: select, split and audit share u(seed, id) without domain separation; v0 with `--limit 100` lands entirely in held-out.
6. downstream dataset consumer SR-002 FND-001: reference packets must hold no model label, but the downstream dataset consumer review rows always carry latest labels.

Non-blocking but due before the milestone that builds them: the Ollama prompt-cache effect on prompt_eval_count (sapho SR-038 FND-001, verify on the local inference host before SAPHO-32), same-weights aliases counted as independent sources (downstream dataset consumer SR-006 FND-002, before the downstream tickets that build it), the all-true Boolean claim export (downstream dataset consumer SR-009 FND-001, before the downstream tickets that build it), and the open items below.

## Validation

`quire validate --scope . 'spec/**/*.md'` (quire 0.36.1, engine 0.50.1) exits 0 in all three repos at the reviewed heads; `--summary` reports sapho 168/168 and the two downstream consumers 35/35 and 19/20 documents grammar-clean (the latter with one ears:non-singular warning at spec/functional/FR-001.md:34). Every run prints the environmental advisories DuplicateArchetype (ADR, Plan, Review, SpecReview, Standard from spec-artifacts-process twice), DuplicateInverseEdge (part_of) and semantic.inline-data-schema; they do not affect document results. IDs are unique per repo, every relative link and every ix:// target (in-repo and cross-repo) resolves, and no FR, NFR or IT is orphaned. A grep of each diff finds no deprecated item (framed encodings, preflight certificates, byte-as-token guard, exclusion registries, Projection v1, Python, ears-campaign), no task-log wording, and no Sapho reference to the downstream dataset consumer.

## Dispositions

Round 1, reviewed at agent-ix/sapho@2812aa1800068709943743661d947c4d1ad21692.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-001 | fixed | 2812aa1: New FR-054 specifies the Ollama ModelBackend (format enum + logprobs); ollama spec.md In Scope and master spec updated; TC-054 added. |

## New findings (disposition pass 2)

Reviewed at agent-ix/sapho@a34c54c8909d6f73a6a2258e4419fa2fa5b0af38.

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-002 | low | Reviewer artifact: SR-033's quoted excerpt held a relative link that broke when the file moved to spec/reviews/ | spec/reviews/sapho-75-spec-review.md:29 |

### FND-002 (low, confidence high, other)

Unit: ollama/spec.md#In-Scope at spec/reviews/sapho-75-spec-review.md:29. Related: .

> > The [Extractor](../modules/core/functional/FR-048.md) implementation over Ollama's generate endpoint; explicit generation settings per binding; token estimation before a call and the reported token count after it; a `TooLarge` outcome; one request in flight per process; model identity, digest, usage and raw exchange bytes on every response; embeddings over Ollama's embed endpoint; host-configured endpoint, timeout and byte ceilings.

Reviewer artifact: SR-033's quoted excerpt held a relative link that broke when the file moved to spec/reviews/. The FND-001 excerpt quoted ollama/spec.md verbatim including the relative link `../core/functional/FR-048.md`, which resolves from spec/modules/ollama/ but not from spec/reviews/. The reviewer repaired only the link path in this file's Analysis excerpt (now `../modules/core/functional/FR-048.md`); the Findings table and finding text are unchanged.

## Dispositions (round 3)

Round 3, reviewed at agent-ix/sapho@ae955d0243b5cc036dce878e13cad28277321108.

| FND | outcome | sha/reason |
|-----|---------|------------|
| FND-002 | deferred | Reviewer artifact. The round-2 excerpt of this same finding re-quoted the link `../core/functional/FR-048.md`, so the committed file still has one broken relative link. Repaired in the round-3 scratchpad copy of this SR file, together with the same kind of quoted relative links in the downstream consumers' SR-002, SR-006 and SR-008 excerpts and the other consumer's SR-002 excerpt (paths rewritten to resolve from reviews/); lands when the lead commits the round-3 SR files. Link paths only; no finding text changed. |
