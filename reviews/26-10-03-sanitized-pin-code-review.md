---
id: SR-030
title: "Code and Rust review of the sanitized ix-cli-kit pin"
type: SpecReview
analysis: code-review
scope: "sapho@945f3ca10ac4698f97104f1f92f0e810120c402e versus main@a539adc91d0e23c648351cf2f48c9fea92381f84; entire Cargo.toml/Cargo.lock diff, authoritative dependency Git blob equivalence, repository Rust conventions, unchanged source/tests/specifications, Makefile/CI and deny policy"
review_set: subset
---
# SR-030: Sanitized dependency pin review

## Summary

Reviewed the complete dependency transition to the approved sanitized ix-cli-kit revision. Only the manifest revision and its lockfile source identity change; the dependency's runtime, tests, dependency manifests and build gates have identical authoritative Git blobs to the old revision.

## Verdict

PASS for the consumer pin transition. This self-review does not assert anonymous access or readiness to publish the dependency; both repositories remain private while GitHub's retained historical pages are resolved.

## Findings

| ID | Severity | Summary | Refs |
|---|---|---|---|
| FND-001 | low | No findings (placeholder) | - |

## Analysis

No applicable AssuranceProfile exists. The Rust checklist and committed repository idioms were applied to the manifest change: no Rust implementation, API, source boundary, credential behavior, async/blocking discipline, resource limit, test seam, criterion trace, unsafe code or gate changes. The dependency remains directly referenced through its authoritative Git repository, with the same secrets feature. No copied source, compatibility layer, fallback revision, temporary shared checkout edit or advisory exemption was added.

GitHub's authoritative trees prove identity of twenty-three runtime/test/dependency/gate blobs between the old pin and 8e6781eb39072bbdb43c8e7d19ea3ed216959897. Cargo fetched the new revision from the existing private repository. A structured comparison of the lockfile packages proves every other package/version/dependency edge unchanged. Cargo's incidental refresh of Windows resolution edges was excluded; the final two-line pin delta passed `cargo check --locked --workspace --all-features` and the full `make ci` before PR creation. All 84 tests and one doctest passed in each feature lane, alongside formatting, Clippy, docs, dependency/license/advisory audit, unsafe audit and spec/review validation. No tests were added for a source-identity-only change.

## Delivery verification

The second full `make ci` exited 0 before merge, again passing 84 tests and one doctest per feature lane and all lint, dependency, docs, unsafe and spec gates. The producer subsequently became public under Peter's explicit approval. Independent anonymous GitHub API, exact-revision HTTPS Git fetch and pinned-manifest checks passed, with credentials and global/system Git configuration disabled for the fetch. The temporary verification repository was removed. PR #12 passes the required CLA checks. The earlier private-repository statements describe conditions at review; the dependency-publication blocker is now resolved.
