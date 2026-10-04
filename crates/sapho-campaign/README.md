# Sapho campaign host

Reusable durable orchestration around the existing native graph runtime. Storage,
lifecycle, controls, adapter, execution, runner and export have separate modules.
No Quire dependency, EARS semantics, campaign data or Git publication lives here.

`CampaignAdapter` is a trusted compiled Rust extension, not a sandbox. Admission
and preparation run synchronously. Async execution receives owned preparation and
backend bindings, never a SQLite connection. Domain sealing and generic completion
commit in one fenced transaction. Domain acceptance/completion remain separate from
engine success. `GraphAdapter` supplies the stock native Inputs/GraphSpec path.

Use `sapho campaign --state PATH <command>`: init, import --job JSON, run,
status, tui, pause, resume, retry --attempt N --reason TEXT, export --output NEW_DIR,
doctor. Jobs use schema 1, adapter `sapho-graph/v1`, payload_schema 1 and a payload
with native graph, typed inputs and explicit positive limits. Graphs remain native
Sapho YAML/JSON; parsing does not introduce another workflow engine.

Local inference: run --ollama-url http://127.0.0.1:11434 --model MODEL. Context and
output ceilings are explicit; no model is installed or downloaded. Alternatively
use existing CLI binding documents or an exact --recording. Credentials never
enter job artifacts. Run does not automatically retry or repair failed attempts.

The TUI is the separate sapho-campaign-tui crate. It reads snapshots, queues guarded
controls and restores the terminal on detach/error. The headless worker continues.
A queued receipt is not proof the control was applied. Pause uses control revision;
retry uses exact attempt/state and keeps the original outcome in all denominators.

Default tests are synthetic and offline. A completed graph says nothing about gold
correctness, novelty or human usefulness. Consumer adaptation, migration and final
review are required before this extraction is handed off as complete.
