---
id: SR-061
title: "Base review of SAPHO-31 System One service configuration"
type: SpecReview
analysis: base
scope: "SAPHO-31; spec/spec.md; FR-072..073; IT-013; CLI module index"
review_set: base
---
# SR-061: Base review of SAPHO-31 System One service configuration

## Summary

The scoped requirements add one generic stock `systemone` binding kind backed by existing CLM transport and shared System One codec, with per-binding base URL and OS-store credential reference in separate host configuration. This review checked all six ticket acceptance checks, two- and three-service dispatch, limits and URL policy, secret boundaries, replay isolation, legacy Jev/CLM compatibility and the EARS/Dataset boundary. No implementation or model-quality result is claimed.

## Findings

| ID | Severity | Summary | Refs |
|----|----------|---------|------|
| FND-001 | low | Existing bindings deliberately reject endpoint and credential fields. Per-binding URL and optional OS-store key now live in a separate host file selected by path; graph/binding schemas stay credential-free. | FR-072 |
| FND-002 | low | `systemone` reuses CLM transport and codec but requires an explicit model; CLM's legacy default, environment endpoint and credential precedence remain unchanged. | FR-072; FR-073 |
| FND-003 | low | Arbitrary graph data literals can contain text; the compiler cannot recognize every secret string. The host rejects credential/endpoint fields in provider configuration positions and never treats a graph literal as service configuration. | FR-072 |
| FND-004 | low | The 11 new acceptance criteria remain untagged until observable implementation tests bind them. | FR-072; FR-073 |

## Review Evidence

`BindingConfig` currently has only Jev/Clm variants and denies unknown fields. `live_bindings` prepares required providers, while `replay_bindings` uses recorded BackendId and model/expected-model/policy without a provider. `RecordingBackend` retains typed exchanges, and `sapho-clm` already enforces a 30-second deadline, 1 MiB request, 8 MiB response, four in-flight calls and remote HTTPS with loopback HTTP. IT-013 uses two independent listener captures, a third name supplied only by files, a secret-store sentinel and offline replay to make the boundary observable. Existing Jev/CLM behavior is pinned by regression checks. Dataset JSON/serde/validate and EARS production remain untouched.

`quire validate --scope . 'spec/**/*.md'` exited 0 with installed catalog diagnostics only; `git diff --check` exited 0; `quire matrix --scope . --format tsv` reported 11 scoped criteria `untagged`. No Rust tests, real service, EARS private data or live passage run are claimed for this specification stage.
