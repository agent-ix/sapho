---
id: SR-026
title: "Code and Rust review of CLM and shared CLI providers"
type: SpecReview
analysis: code-review
scope: "sapho@85a14b5b4a319d383aa10e2f18ac57f2a1e8158a versus main@fe8ab2c2eb944a31de58de29134ae3ff906c45e3; entire PR diff: manifests/lockfile/deny.toml, CLAUDE.md, sapho-clm and sapho-systemone source/tests, CLI bindings/credentials/errors/host/process and both integration suites, core Usage, Jev translator/tests, Unix selector tests, facade/runtime scenario tags, README/guides and provider specifications/reviews; unchanged Makefile and CI examined"
review_set: subset
---
# SR-026: Provider code and Rust review

## Summary

Reviewed the complete provider change against the repository conventions, code-review and Rust-review checklists. CLM uses the verified System One request vocabulary, preserves provider evidence, and has a real bounded HTTP adapter; CLI credential acquisition remains outside asynchronous evaluation and offline paths.

## Verdict

PASS. No unresolved code, Rust, boundary, duplication, or test-integrity findings at the reviewed source revision. This is a self-review; no independent reviewer or live model evaluation is claimed.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Analysis

No applicable AssuranceProfile is present. Repository conventions, both feature lanes, lint policy, unsafe gate and complete-workspace dependency audit remain intact. License additions are restricted to the two new Sapho crates and the authorized AGPL ix-cli-kit dependency; its Git source is explicitly allowed and revision-pinned. No advisory exception, dependency source copy, consumer evidence, model weights or compatibility reader was introduced.

The request translator has one owner in sapho-systemone and uses SDK-owned SystemOneRequest, Questions and NoulCriteria types. Jev's existing exported function refers to that owner. The CLM model alias has one implementation owner shared by the adapter and CLI, including offline minimal builds. CLM owns its response decoder because its strict schema and billing units differ from the Jev SDK response. These original adapter types implement the user-authorized protocol; no CLM source or schema artifact was copied. Research used CLM main bb42c6c5bf914fd449bed2f6ca65be80602cb1f7 and the authoritative model card. Decisions has no fabricated endpoint, substitute backend or stub.

HTTP initialization accepts only bounded positive configuration and a safe HTTP(S) base URL, restricts cleartext to loopback, and rejects embedded userinfo/query/fragment. Sensitive authorization headers are never evidence. Redirects and retries are disabled. Advertised and streamed body lengths are checked; request serialization refuses before send. A finite deadline includes queueing and decode, with explicit expired-deadline checks before sending and before success. Semaphore permits release on timeout or cancellation; no lock spans await. Native HTTP errors and error response bodies cannot reach diagnostics.

Response decoding rejects duplicate keys, unknown fields, malformed types, mismatched kinds/IDs, incomplete distributions, invalid probabilities and score legends. Semantic selected-label, expected-score, mass and expected-model checks remain core/runtime responsibilities, preserving valid-wire raw evidence for failures. Confidence 0.4 and top probability 0.6 are independently retained in a three-option response; fractional scores and question charges survive exact recording/replay. No normalization or confidence recomputation occurs in the adapter.

The CLI depends directly on ix-cli-kit cc69a934f887966f955440bff8c356e3da449bee. Shared secret resolution enforces explicit/environment/OS precedence with typed locked/unavailable/backend errors, redacted values and no file fallback. Acquisition is synchronous before Tokio execution. Only required live bindings prepare transports. Jev SDK logging is forced off even when the environment requests debug. Replay compares only model/expected-model/policy identity, never an inferred provider, and works after the loopback service stops with poisoned credential/endpoint environment. Shared streams and Outcome preserve Sapho's existing JSON envelopes, newlines and 0/1/2 exits; existing bounded config parsing stays authoritative. Shared version agreement checks the actual executable without replacing Clap's existing version contract.

Tests use actual public command, compiler/runtime, native HTTP and credential/backend seams. No production test-only bypass, panic, unsafe block, arbitrary script loader or timing-threshold assertion was added. macOS now executes the existing child kill/reap assertions through waitpid instead of a Linux-only procfs assertion. Documentation describes host-managed deployment, exact limits, OS credential identity, optional billing units and the Decisions prerequisite. Downstream Usage literal impact was communicated; no shared checkout was changed.

## Validation

At 85a14b5b4a319d383aa10e2f18ac57f2a1e8158a, `make ci` exited 0: 84 tests plus one doctest in each feature lane; both full-workspace Clippy lanes with warnings denied; format check; locked all-feature workspace cargo-deny; unsafe audit; all-feature Rust docs with warnings denied; specs and existing reviews. Focused CLM/CLI tests and the CLI minimal-feature check also passed. The strict computed matrix reports 149 tagged criteria, three inspection criteria, zero untagged or ignored-only criteria. A release build and the second full pre-merge gate are recorded in the delivery evidence after execution. Ambient quire duplicate-module/archetype and inline-schema advisories are environmental; document validation has no errors.

## Dispositions

No substantive finding requires disposition. The placeholder is schema-required and does not represent a defect.

## Delivery Evidence

The pre-merge `make ci` exited 0 at 5be5e5cf0691def282c74215f9cd37f4e4821050: 84 tests plus one doctest in each feature lane, both Clippy lanes, fmt, locked workspace dependency audit, unsafe audit, warnings-denied Rust docs, specs and both new review artifacts. The production implementation is unchanged from the reviewed 85a14b5b4a319d383aa10e2f18ac57f2a1e8158a. The all-feature release build also exited 0. Both full gates ran entirely on draco with one reused owned target directory and over 220 GiB free disk. GitHub CLA internal-author checks passed; CI's manual source-build workflow was not dispatched to another machine.
