# Sapho

Sapho is a Rust workspace containing core, graph, runtime, Jev, CLM, System One codec, recording, Ollama, evidence, selection and CLI crates.
Read spec/spec.md and the owning module before changing behavior. Specifications
precede implementation; domain adapters and rules remain downstream.

## Rust conventions

Use typed IDs, exhaustive enums, thiserror-derived structured errors and Result.
Forbid unsafe code; no production unwrap/expect/indexing on caller-controlled data.
Borrow inputs; avoid clones except for owned async work or retained evidence.
Use BTreeMap for deterministic named maps and Vec for ordered model criteria.
Validate public deserialized payloads and reject unknown fields. Public APIs and
modules have Rust documentation. Core has no sibling dependency; graph depends
on core; runtime depends on graph/core; Recording and Ollama depend only on core. Jev and CLM depend on core and the shared systemone request codec; systemone depends only on core among workspace crates. Evidence and selection depend only on core among workspace crates; evidence is pure and selection owns bounded host acquisition. CLI coordinates these crates, with synchronous acquisition/persistence outside async evaluation.
No native or filesystem blocking on Tokio workers. Runtime limits cover nested
work. No lock is held across await. Tests use Trace: acceptance-criterion tags,
assert observable values, and inject doubles only at native/backend seams.

## Commands

Rust is pinned to 1.98.1. Use one workspace target directory.
make fmt; make lint; make test; make docs; make deny; make ci.
Spec gate: quire validate --scope . "spec/**/*.md".
Default tests require no network, model server or credential.
