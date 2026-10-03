---
id: SR-028
title: "Code review of Sapho public-readiness metadata"
type: SpecReview
analysis: code-review
scope: "sapho@573d45cb63d1b220303517bde247cb0bea436dce versus main@2e348c9336339d078ec6f9430d3cf1feadcd97a0; complete documentation diff in README.md, CONTRIBUTING.md and CONTENT_RIGHTS.md; unchanged license, canonical CLA, manifests, source SPDX notices, workflows, CODEOWNERS, Rust conventions, dependency policy, main protection and public repository metadata examined"
review_set: subset
---
# SR-028: Public-readiness code review

## Summary

Reviewed the complete metadata change and its correspondence to observed repository settings. The expired Discord invite is replaced with Peter's verified Agent IX invite; the package-count claim is removed, and contributor instructions reflect the newly required CLA checks.

## Verdict

PASS for this documentation change. Public visibility remains conditional on the separately owned ix-cli-kit dependency becoming anonymously accessible. This is a self-review, not an independent review or legal opinion.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Analysis

No applicable AssuranceProfile exists. No Rust code, API, specification, test, lockfile, workflow or gate changed. The Rust review checklist was loaded; code-specific checks have no changed Rust surface. No duplicated implementation, vendored content, compatibility reader, new secret or model artifact was introduced. The canonical CLA remains byte-identical to agent-ix/.github (blob cc53694c6001611c548582c0bdc06df50613b41d). All eleven manifests declare AGPL-3.0-or-later, all tracked Rust sources retain SPDX notices, and the license matches the SPDX AGPL reference after whitespace and HTTP-to-HTTPS URL differences.

The Discord API resolves k8DVhuYBR2 to Agent IX with expires_at null. README license and community badges point to their actual destinations. Main requires `cla / gate` and `cla / cla-check`, with strict status checks; existing code-owner approval, stale-review dismissal, force-push/deletion refusal and administrator bypass were preserved. The shared CLA action fails unsigned external contributions; internal exemption requires the verified gate. No privileged workflow checks out or executes contributor code.

## Verification

The first `make ci` exited 0 on draco: formatting, Clippy with warnings denied in both feature lanes, 84 tests and one doctest per lane, dependency/advisory/license audit, unsafe audit, rustdoc and spec/review validation. `git diff --check` passed. No tests were added for link and prose edits; the live invite, native repository settings and canonical documents were inspected directly. Pending uncommitted documentation in the primary checkout is outside this reviewed revision and was preserved.

## Delivery verification

The second full `make ci` also exited 0 before handoff, with the same 84 tests and one doctest in each feature lane. PR #11 passes the required CLA gate and verified internal-contribution exemption. Only review execution evidence and trailing whitespace changed after the reviewed documentation revision. Public visibility remains held for the separately owned sanitized ix-cli-kit release and consumer pin transition.
